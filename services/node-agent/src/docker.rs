//! Docker Engine adapter. Isolated so the rest of the agent does not shell out
//! to the Docker CLI.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bollard::exec::StartExecResults;
use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, ExecConfig, HostConfig, PortBinding, PortMap,
    Volume,
};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, InspectContainerOptions,
    ListContainersOptionsBuilder, LogsOptionsBuilder, StartContainerOptions, StopContainerOptions,
    WaitContainerOptions,
};
use bollard::Docker;
use fps_branding::PACKAGE_NAME;
use fps_domain::{DockerState, ServerId};
use fps_protocol::{DockerCapability, LogChunk};
use futures_util::StreamExt;
use tracing::debug;

pub async fn probe() -> DockerCapability {
    match Docker::connect_with_defaults() {
        Ok(docker) => match docker.version().await {
            Ok(version) => DockerCapability {
                state: DockerState::Available,
                engine_version: version.version,
                api_version: version.api_version,
                cgroup_version: None,
                error: None,
            },
            Err(err) => {
                debug!(error = %err, "docker version failed");
                DockerCapability {
                    state: DockerState::Error,
                    engine_version: None,
                    api_version: None,
                    cgroup_version: None,
                    error: Some(redact(&err.to_string())),
                }
            }
        },
        Err(err) => DockerCapability {
            state: DockerState::Unavailable,
            engine_version: None,
            api_version: None,
            cgroup_version: None,
            error: Some(redact(&err.to_string())),
        },
    }
}

pub async fn run_disposable() -> anyhow::Result<String> {
    let docker = Docker::connect_with_defaults()?;
    let mut pull = docker.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image("hello-world")
                .build(),
        ),
        None,
        None,
    );
    while let Some(item) = pull.next().await {
        item?;
    }
    let created = docker
        .create_container(
            None::<bollard::query_parameters::CreateContainerOptions>,
            ContainerCreateBody {
                image: Some("hello-world".into()),
                host_config: Some(HostConfig {
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await?;
    docker
        .start_container(&created.id, None::<StartContainerOptions>)
        .await?;
    let mut wait = docker.wait_container(&created.id, None::<WaitContainerOptions>);
    let mut status = 0i64;
    while let Some(item) = wait.next().await {
        status = item?.status_code;
    }
    Ok(format!(
        "docker-ok container={} status={status}",
        created.id
    ))
}

pub fn server_id_label_key() -> String {
    format!("{PACKAGE_NAME}.server-id")
}

pub fn connect() -> Result<Docker, String> {
    Docker::connect_with_defaults().map_err(|err| redact(&err.to_string()))
}

pub async fn pull_image(docker: &Docker, image: &str) -> Result<(), String> {
    let mut pull = docker.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image(image)
                .build(),
        ),
        None,
        None,
    );
    while let Some(item) = pull.next().await {
        item.map_err(|err| redact(&err.to_string()))?;
    }
    Ok(())
}

pub struct WorkloadCreate {
    pub container_name: String,
    pub image: String,
    pub env: Vec<(String, String)>,
    pub cmd: Option<Vec<String>>,
    pub ports: Vec<PortPublish>,
    pub memory_mb: u64,
    pub host_dir: PathBuf,
    pub volume_path: String,
    pub server_id: String,
}

pub struct PortPublish {
    pub host: u16,
    pub container: u16,
    pub protocol: String,
}

pub async fn create_and_start_workload(
    docker: &Docker,
    spec: &WorkloadCreate,
) -> Result<(String, String), String> {
    let mut labels = HashMap::new();
    labels.insert(server_id_label_key(), spec.server_id.clone());

    let mut port_bindings: PortMap = HashMap::new();
    let mut exposed_ports = HashMap::new();
    for port in &spec.ports {
        let proto = if port.protocol.is_empty() {
            "tcp"
        } else {
            port.protocol.as_str()
        };
        let key = format!("{}/{}", port.container, proto);
        exposed_ports.insert(key.clone(), HashMap::new());
        port_bindings.insert(
            key,
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".into()),
                host_port: Some(port.host.to_string()),
            }]),
        );
    }

    let host_dir = absolute_dir(&spec.host_dir);
    let bind = format!("{}:{}", host_dir.display(), spec.volume_path);

    let env = if spec.env.is_empty() {
        None
    } else {
        Some(
            spec.env
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>(),
        )
    };

    let cmd = spec
        .cmd
        .as_ref()
        .and_then(|c| if c.is_empty() { None } else { Some(c.clone()) });

    let body = ContainerCreateBody {
        image: Some(spec.image.clone()),
        env,
        cmd,
        labels: Some(labels),
        exposed_ports: if exposed_ports.is_empty() {
            None
        } else {
            Some(exposed_ports)
        },
        host_config: Some(HostConfig {
            auto_remove: Some(false),
            memory: Some((spec.memory_mb as i64) * 1024 * 1024),
            binds: Some(vec![bind]),
            port_bindings: if port_bindings.is_empty() {
                None
            } else {
                Some(port_bindings)
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    let created = match docker
        .create_container(
            Some(
                CreateContainerOptionsBuilder::default()
                    .name(&spec.container_name)
                    .build(),
            ),
            body,
        )
        .await
    {
        Ok(created) => created,
        Err(err) => {
            let msg = err.to_string();
            if is_name_conflict(&msg) {
                start_named(docker, &spec.container_name).await?;
                let inspected = inspect_named(docker, &spec.container_name).await?;
                let id = inspected.id.unwrap_or_default();
                return Ok((id, spec.container_name.clone()));
            }
            return Err(redact(&msg));
        }
    };

    docker
        .start_container(&created.id, None::<StartContainerOptions>)
        .await
        .map_err(|err| redact(&err.to_string()))?;
    Ok((created.id, spec.container_name.clone()))
}

pub async fn start_named(docker: &Docker, name: &str) -> Result<(), String> {
    docker
        .start_container(name, None::<StartContainerOptions>)
        .await
        .map_err(|err| redact(&err.to_string()))
}

pub async fn stop_named(docker: &Docker, name: &str) -> Result<(), String> {
    docker
        .stop_container(name, None::<StopContainerOptions>)
        .await
        .map_err(|err| redact(&err.to_string()))
}

pub async fn inspect_named(
    docker: &Docker,
    name: &str,
) -> Result<ContainerInspectResponse, String> {
    docker
        .inspect_container(name, None::<InspectContainerOptions>)
        .await
        .map_err(|err| redact(&err.to_string()))
}

pub fn container_is_running(inspect: &ContainerInspectResponse) -> bool {
    inspect
        .state
        .as_ref()
        .and_then(|s| s.running)
        .unwrap_or(false)
}

pub async fn inspect_named_volume(docker: &Docker, name: &str) -> Result<Volume, String> {
    docker
        .inspect_volume(name)
        .await
        .map_err(|err| redact(&err.to_string()))
}

pub async fn exec_ls(docker: &Docker, container_name: &str, path: &str) -> Result<String, String> {
    let created = docker
        .create_exec(
            container_name,
            ExecConfig {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["ls".into(), "-la".into(), path.into()]),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| redact(&err.to_string()))?;
    match docker
        .start_exec(&created.id, None)
        .await
        .map_err(|err| redact(&err.to_string()))?
    {
        StartExecResults::Attached { mut output, .. } => {
            let mut text = String::new();
            while let Some(item) = output.next().await {
                match item {
                    Ok(chunk) => text.push_str(&chunk.to_string()),
                    Err(err) => return Err(redact(&err.to_string())),
                }
                if text.len() > 16_384 {
                    break;
                }
            }
            Ok(text)
        }
        StartExecResults::Detached => Ok(String::new()),
    }
}

pub async fn exec_shell(
    docker: &Docker,
    container_name: &str,
    command: &str,
) -> Result<String, String> {
    let created = docker
        .create_exec(
            container_name,
            ExecConfig {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["sh".into(), "-c".into(), command.into()]),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| redact(&err.to_string()))?;
    match docker
        .start_exec(&created.id, None)
        .await
        .map_err(|err| redact(&err.to_string()))?
    {
        StartExecResults::Attached { mut output, .. } => {
            let mut text = String::new();
            while let Some(item) = output.next().await {
                match item {
                    Ok(chunk) => text.push_str(&chunk.to_string()),
                    Err(err) => return Err(redact(&err.to_string())),
                }
                if text.len() > 16_384 {
                    break;
                }
            }
            Ok(text)
        }
        StartExecResults::Detached => Ok(String::new()),
    }
}

pub async fn collect_container_samples() -> Vec<fps_protocol::ContainerSample> {
    let docker = match connect() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut samples = Vec::new();
    for tracked in list_labeled_containers().await {
        let Some(server_id) = tracked.server_id else {
            continue;
        };
        match inspect_named(&docker, &tracked.name).await {
            Ok(inspect) => {
                let running = container_is_running(&inspect);
                let memory_bytes = inspect
                    .host_config
                    .as_ref()
                    .and_then(|h| h.memory)
                    .map(|m| m as u64);
                let restart_count = inspect.restart_count.unwrap_or(0) as u32;
                samples.push(fps_protocol::ContainerSample {
                    server_id,
                    running,
                    memory_bytes,
                    cpu_percent: None,
                    restart_count,
                });
            }
            Err(_) => samples.push(fps_protocol::ContainerSample {
                server_id,
                running: false,
                memory_bytes: None,
                cpu_percent: None,
                restart_count: 0,
            }),
        }
    }
    samples
}

pub async fn tail_logs(
    docker: &Docker,
    container_name: &str,
    tail: &str,
) -> Result<String, String> {
    let opts = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .follow(false)
        .tail(tail)
        .build();
    let mut stream = docker.logs(container_name, Some(opts));
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => text.push_str(&chunk.to_string()),
            Err(err) => return Err(redact(&err.to_string())),
        }
        if text.len() > 8_192 {
            break;
        }
    }
    Ok(text)
}

pub async fn count_labeled_workloads() -> u32 {
    list_labeled_containers().await.len() as u32
}

pub struct TrackedContainer {
    pub name: String,
    pub server_id: Option<ServerId>,
}

pub async fn list_labeled_containers() -> Vec<TrackedContainer> {
    let docker = match connect() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let label = server_id_label_key();
    let mut filters: HashMap<String, Vec<String>> = HashMap::new();
    filters.insert("label".into(), vec![label.clone()]);
    let opts = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let listed = match docker.list_containers(Some(opts)).await {
        Ok(v) => v,
        Err(err) => {
            debug!(error = %err, "list labeled containers failed");
            return Vec::new();
        }
    };
    listed
        .into_iter()
        .filter_map(|c| {
            let name = c
                .names
                .as_ref()
                .and_then(|n| n.first())
                .map(|n| n.trim_start_matches('/').to_string())
                .filter(|n| !n.is_empty())?;
            let server_id = c
                .labels
                .as_ref()
                .and_then(|labels| labels.get(&label))
                .and_then(|v| v.parse().ok());
            Some(TrackedContainer { name, server_id })
        })
        .collect()
}

pub async fn collect_workload_logs() -> Vec<LogChunk> {
    let docker = match connect() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut chunks = Vec::new();
    for tracked in list_labeled_containers().await {
        let Some(server_id) = tracked.server_id else {
            continue;
        };
        match tail_logs(&docker, &tracked.name, "40").await {
            Ok(text) if !text.trim().is_empty() => chunks.push(LogChunk {
                server_id,
                stream: "stdout".into(),
                text,
            }),
            _ => {}
        }
    }
    chunks
}

pub fn volume_host_dir(data_dir: &Path, container_name: &str) -> PathBuf {
    data_dir.join("volumes").join(container_name)
}

pub fn backup_archive_path(data_dir: &Path, backup_id: &str) -> PathBuf {
    data_dir.join("backups").join(format!("{backup_id}.tgz"))
}

pub(crate) fn redact(msg: &str) -> String {
    match msg.char_indices().nth(300) {
        Some((idx, _)) => format!("{}…", &msg[..idx]),
        None => msg.to_string(),
    }
}

fn is_name_conflict(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("409") || lower.contains("conflict") || lower.contains("already in use")
}

fn absolute_dir(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path),
        Err(_) => path.to_path_buf(),
    }
}
