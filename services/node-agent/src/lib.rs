pub mod addons;
pub mod docker;
pub mod identity;
pub mod jobs;
pub mod sys;

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use fps_branding::{PACKAGE_NAME, VERSION};
use fps_domain::ObservedResources;
use fps_protocol::{
    DockerCapability, EnrollRequest, EnrollResponse, HeartbeatRequest, HeartbeatResponse,
    JobResult, LogChunk, NodeControlAck, PROTOCOL_VERSION,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::identity::NodeIdentity;

#[derive(Clone)]
pub struct AgentConfig {
    pub control_plane_url: String,
    pub data_dir: std::path::PathBuf,
    pub name: Option<String>,
    pub labels: Vec<String>,
    pub heartbeat_interval: Duration,
    pub allow_insecure_http: bool,
}

pub struct AgentRuntime {
    pub pending_results: Mutex<Vec<JobResult>>,
    pub pending_logs: Mutex<Vec<LogChunk>>,
    pub pending_ack: Mutex<Option<NodeControlAck>>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self {
            pending_results: Mutex::new(Vec::new()),
            pending_logs: Mutex::new(Vec::new()),
            pending_ack: Mutex::new(None),
        }
    }
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self::default()
    }
}

pub async fn enroll(cfg: &AgentConfig, token: &str) -> Result<NodeIdentity> {
    let docker = docker::probe().await;
    let resources = sys::observe();
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown-host".into());
    let req = EnrollRequest {
        enrollment_token: token.to_string(),
        hostname: hostname.clone(),
        name: cfg.name.clone(),
        agent_version: VERSION.to_string(),
        protocol_version: PROTOCOL_VERSION,
        architecture: std::env::consts::ARCH.to_string(),
        operating_system: std::env::consts::OS.to_string(),
        labels: cfg.labels.clone(),
        docker,
        resources,
    };
    let client = public_http_client(cfg.allow_insecure_http, &cfg.control_plane_url)?;
    let url = format!(
        "{}/v1/nodes/enroll",
        cfg.control_plane_url.trim_end_matches('/')
    );
    let response = client
        .post(&url)
        .header("user-agent", fps_branding::user_agent())
        .json(&req)
        .send()
        .await
        .context("enroll request")?;
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("enrollment failed: {body}");
    }
    let enrolled: EnrollResponse = response.json().await.context("enroll json")?;
    let identity = NodeIdentity {
        node_id: enrolled.node_id.to_string(),
        node_token: enrolled.node_token,
        certificate_pem: enrolled.certificate_pem,
        private_key_pem: enrolled.private_key_pem,
        ca_pem: enrolled.ca_pem,
        control_plane_url: cfg.control_plane_url.clone(),
        node_endpoint: enrolled.node_endpoint,
        heartbeat_interval_seconds: enrolled.heartbeat_interval_seconds,
    };
    identity.save(&cfg.data_dir)?;
    info!(node_id = %identity.node_id, hostname, "node enrolled");
    Ok(identity)
}

pub async fn run_heartbeat_loop(cfg: &AgentConfig, identity: &NodeIdentity) -> Result<()> {
    let started_at = chrono::Utc::now();
    let mut interval = Duration::from_secs(identity.heartbeat_interval_seconds.max(5));
    let mut identity = identity.clone();
    let runtime = AgentRuntime::new();
    loop {
        match send_heartbeat_tick(cfg, &identity, started_at, &runtime).await {
            Ok(resp) => {
                if !resp.accepted {
                    warn!("heartbeat was not accepted");
                }
                if let Some(secs) = resp.settings.heartbeat_interval_seconds {
                    let secs = secs.max(5);
                    if secs != identity.heartbeat_interval_seconds {
                        identity.heartbeat_interval_seconds = secs;
                        let _ = identity.save(&cfg.data_dir);
                        interval = Duration::from_secs(secs);
                        info!(secs, "heartbeat interval updated from control plane");
                    }
                }
                if resp.settings.docker_prune {
                    match docker::prune_unused().await {
                        Ok(msg) => {
                            info!(%msg, "docker prune completed");
                            *runtime.pending_ack.lock().await = Some(NodeControlAck {
                                docker_prune: Some("completed".into()),
                                uninstall: None,
                            });
                        }
                        Err(err) => warn!(error = %err, "docker prune failed"),
                    }
                }
                if resp.settings.uninstall {
                    let stopped = docker::stop_all_labeled().await;
                    info!(?stopped, "stopping workloads; uninstalling agent");
                    let ack = NodeControlAck {
                        uninstall: Some("completed".into()),
                        docker_prune: None,
                    };
                    let _ = send_heartbeat_ex(
                        cfg,
                        &identity,
                        started_at,
                        Vec::new(),
                        Vec::new(),
                        Some(ack),
                    )
                    .await;
                    finish_uninstall(&cfg.data_dir);
                    info!("agent uninstalled at control-plane request");
                    return Ok(());
                }
                for job in resp.jobs {
                    let result = jobs::execute(&cfg.data_dir, &job).await;
                    runtime.pending_results.lock().await.push(result);
                }
            }
            Err(err) => {
                warn!(error = %err, "heartbeat failed; will retry");
            }
        }
        tokio::time::sleep(interval).await;
    }
}

fn finish_uninstall(data_dir: &Path) {
    for name in ["identity.json", "node.key", "node.crt", "ca.crt"] {
        let _ = std::fs::remove_file(data_dir.join(name));
    }
    if Path::new("/run/systemd/system").exists() {
        let _ = std::process::Command::new("systemctl")
            .args(["disable", "--now", "fps-node-agent.service"])
            .spawn();
    }
}

/// Compatible heartbeat used by integration tests: empty job results and log chunks.
pub async fn send_heartbeat(
    cfg: &AgentConfig,
    identity: &NodeIdentity,
    started_at: chrono::DateTime<chrono::Utc>,
) -> Result<HeartbeatResponse> {
    send_heartbeat_ex(cfg, identity, started_at, Vec::new(), Vec::new(), None).await
}

pub async fn send_heartbeat_ex(
    cfg: &AgentConfig,
    identity: &NodeIdentity,
    started_at: chrono::DateTime<chrono::Utc>,
    job_results: Vec<JobResult>,
    log_chunks: Vec<LogChunk>,
    control_ack: Option<NodeControlAck>,
) -> Result<HeartbeatResponse> {
    let docker = docker::probe().await;
    let resources = sys::observe();
    let workload_count = docker::count_labeled_workloads().await;
    let body = HeartbeatRequest {
        protocol_version: PROTOCOL_VERSION,
        agent_version: VERSION.to_string(),
        docker,
        resources,
        started_at,
        workload_count,
        note: Some(format!("{PACKAGE_NAME} agent heartbeat")),
        job_results,
        log_chunks,
        container_samples: docker::collect_container_samples().await,
        control_ack,
    };
    let base = identity.heartbeat_base_url().trim_end_matches('/');
    let url = format!("{base}/v1/nodes/{}/heartbeat", identity.node_id);
    let client = heartbeat_client(cfg, identity, base)?;
    let mut request = client
        .post(&url)
        .header("user-agent", fps_branding::user_agent())
        .json(&body);
    if !base.starts_with("https://") {
        request = request.bearer_auth(&identity.node_token);
    }
    let response = request.send().await.context("heartbeat request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("heartbeat {status}: {body}");
    }
    Ok(response.json().await?)
}

async fn send_heartbeat_tick(
    cfg: &AgentConfig,
    identity: &NodeIdentity,
    started_at: chrono::DateTime<chrono::Utc>,
    runtime: &AgentRuntime,
) -> Result<HeartbeatResponse> {
    let job_results = {
        let mut pending = runtime.pending_results.lock().await;
        std::mem::take(&mut *pending)
    };
    let mut log_chunks = {
        let mut pending = runtime.pending_logs.lock().await;
        std::mem::take(&mut *pending)
    };
    if log_chunks.is_empty() {
        log_chunks = docker::collect_workload_logs().await;
    }
    let control_ack = {
        let mut pending = runtime.pending_ack.lock().await;
        pending.take()
    };
    send_heartbeat_ex(
        cfg,
        identity,
        started_at,
        job_results,
        log_chunks,
        control_ack,
    )
    .await
}

pub async fn disposable_workload_probe() -> Result<String> {
    docker::run_disposable().await
}

fn public_http_client(allow_insecure: bool, url: &str) -> Result<Client> {
    if url.starts_with("http://") && !allow_insecure {
        bail!("refusing HTTP control-plane URL unless --allow-insecure-http is set");
    }
    let mut builder = Client::builder()
        .user_agent(fps_branding::user_agent())
        .timeout(Duration::from_secs(15));
    if allow_insecure && url.starts_with("https://") {
        builder = builder.danger_accept_invalid_certs(true);
    }
    Ok(builder.build()?)
}

fn heartbeat_client(cfg: &AgentConfig, identity: &NodeIdentity, base: &str) -> Result<Client> {
    let mut builder = Client::builder()
        .user_agent(fps_branding::user_agent())
        .timeout(Duration::from_secs(15));
    if base.starts_with("https://") {
        let pem = format!("{}{}", identity.certificate_pem, identity.private_key_pem);
        let id = reqwest::Identity::from_pem(pem.as_bytes()).context("node client identity")?;
        let ca = reqwest::Certificate::from_pem(identity.ca_pem.as_bytes())
            .context("node CA certificate")?;
        builder = builder
            .identity(id)
            .add_root_certificate(ca)
            .https_only(true);
    } else if base.starts_with("http://") {
        if !cfg.allow_insecure_http {
            bail!("refusing HTTP heartbeat unless --allow-insecure-http is set");
        }
    } else {
        bail!("unsupported node endpoint '{base}'");
    }
    Ok(builder.build()?)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentStateFile {
    pub identity: NodeIdentity,
}

pub fn load_identity(data_dir: &Path) -> Result<NodeIdentity> {
    NodeIdentity::load(data_dir)
}

pub fn observe_resources() -> ObservedResources {
    sys::observe()
}

pub fn docker_capability_blocking() -> DockerCapability {
    DockerCapability {
        state: fps_domain::DockerState::Unavailable,
        engine_version: None,
        api_version: None,
        cgroup_version: None,
        error: Some("use docker::probe() in async context".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_uninstall_removes_identity_material() {
        let dir = std::env::temp_dir().join(format!(
            "fps-agent-uninstall-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["identity.json", "node.key", "node.crt", "ca.crt"] {
            std::fs::write(dir.join(name), b"secret").unwrap();
        }
        finish_uninstall(&dir);
        for name in ["identity.json", "node.key", "node.crt", "ca.crt"] {
            assert!(!dir.join(name).exists(), "{name} should be removed");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
