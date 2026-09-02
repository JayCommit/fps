use std::net::{IpAddr, SocketAddr};

use axum::extract::{Path, State};
use axum::Json;
use chrono::{Duration, Utc};
use fps_auth::{ct_eq_hex, generate_token, hash_token};
use fps_domain::{BackupId, ErrorCode, JobKind, NodeId, Permission, PlatformError, ServerStatus};
use fps_protocol::{
    protocol_compatible, EnrollRequest, EnrollResponse, HeartbeatRequest, HeartbeatResponse,
    JobResult, NodeControlSettings, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{audit, backups, jobs, logs, metrics, nodes, notifications, servers};
use crate::http::error::ApiError;
use crate::http::extractors::{AuthUser, ClientIp, PeerFingerprint};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeView {
    pub id: NodeId,
    pub name: String,
    pub hostname: String,
    pub architecture: Option<String>,
    pub operating_system: Option<String>,
    pub enrolled_at: chrono::DateTime<Utc>,
    pub workload_count: i32,
    pub revoked: bool,
    pub maintenance: bool,
    pub labels: Vec<String>,
    pub docker_engine_version: Option<String>,
    pub docker_error: Option<String>,
    pub heartbeat_interval_seconds: u64,
    pub uninstall_requested: bool,
    pub uninstalled_at: Option<chrono::DateTime<Utc>>,
    pub health: fps_domain::NodeHealth,
}

fn to_view(n: nodes::NodeRecord, timeout: i64) -> NodeView {
    NodeView {
        health: n.health(timeout),
        id: n.id,
        name: n.name,
        hostname: n.hostname,
        architecture: n.architecture,
        operating_system: n.operating_system,
        enrolled_at: n.enrolled_at,
        workload_count: n.workload_count,
        revoked: n.revoked_at.is_some(),
        maintenance: n.maintenance,
        labels: n.labels,
        docker_engine_version: n.docker_engine_version,
        docker_error: n.docker_error,
        heartbeat_interval_seconds: n.heartbeat_interval_seconds,
        uninstall_requested: n.uninstall_requested_at.is_some(),
        uninstalled_at: n.uninstalled_at,
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EnrollmentTokenRequest {
    pub label: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EnrollmentTokenResponse {
    pub token: String,
    pub expires_at: chrono::DateTime<Utc>,
}

#[utoipa::path(get, path = "/v1/nodes", tag = "nodes", responses((status = 200, body = [NodeView])))]
pub async fn list_nodes(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<NodeView>>, ApiError> {
    auth.require(Permission::NodesRead)?;
    let records = nodes::list(&state.pool).await?;
    let timeout = state.config.heartbeat_timeout_secs;
    Ok(Json(
        records.into_iter().map(|n| to_view(n, timeout)).collect(),
    ))
}

#[utoipa::path(get, path = "/v1/nodes/{id}", tag = "nodes", responses((status = 200, body = NodeView), (status = 404)))]
pub async fn get_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<NodeView>, ApiError> {
    auth.require(Permission::NodesRead)?;
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    let n = nodes::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    Ok(Json(to_view(n, state.config.heartbeat_timeout_secs)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchNodeRequest {
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub maintenance: Option<bool>,
    pub heartbeat_interval_seconds: Option<u64>,
}

#[utoipa::path(
    patch,
    path = "/v1/nodes/{id}",
    tag = "nodes",
    request_body = PatchNodeRequest,
    responses((status = 200, body = NodeView), (status = 400), (status = 404))
)]
pub async fn patch_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PatchNodeRequest>,
) -> Result<Json<NodeView>, ApiError> {
    auth.require(Permission::NodesWrite)?;
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    let node = nodes::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    if node.revoked_at.is_some() {
        return Err(ApiError(PlatformError::new(
            ErrorCode::NodeUntrusted,
            "This node has been revoked.",
        )));
    }
    let name = match body.name {
        Some(n) => {
            let n = n.trim().to_string();
            if n.is_empty() || n.len() > 128 {
                return Err(ApiError(
                    PlatformError::validation("name must be 1–128 characters").field("name"),
                ));
            }
            n
        }
        None => node.name.clone(),
    };
    let labels = match body.labels {
        Some(labels) => {
            if labels.len() > 32 || labels.iter().any(|l| l.is_empty() || l.len() > 64) {
                return Err(ApiError(PlatformError::validation(
                    "at most 32 labels, each 1–64 characters",
                )));
            }
            labels
        }
        None => node.labels.clone(),
    };
    let heartbeat_interval_seconds = match body.heartbeat_interval_seconds {
        Some(secs) if (5..=300).contains(&secs) => secs,
        Some(_) => {
            return Err(ApiError(PlatformError::validation(
                "heartbeat interval must be between 5 and 300 seconds",
            )));
        }
        None => node.heartbeat_interval_seconds,
    };
    let maintenance = body.maintenance.unwrap_or(node.maintenance);
    nodes::update_settings(
        &state.pool,
        id,
        &nodes::NodeSettingsUpdate {
            name: &name,
            labels: &labels,
            maintenance,
            heartbeat_interval_seconds,
        },
    )
    .await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        Some(id),
        "nodes.settings.updated",
        "node",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({
            "name": name,
            "maintenance": maintenance,
            "heartbeat_interval_seconds": heartbeat_interval_seconds,
            "labels": labels,
        }),
    )
    .await?;
    let updated = nodes::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    Ok(Json(to_view(updated, state.config.heartbeat_timeout_secs)))
}

#[utoipa::path(post, path = "/v1/nodes/{id}/uninstall", tag = "nodes", responses((status = 200), (status = 404)))]
pub async fn uninstall_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth.require(Permission::NodesWrite)?;
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    let node = nodes::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    if node.revoked_at.is_some() {
        return Ok(Json(serde_json::json!({
            "ok": true,
            "already_revoked": true,
            "uninstall_requested": false
        })));
    }
    nodes::request_uninstall(&state.pool, id).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        Some(id),
        "nodes.uninstall.requested",
        "node",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "hostname": node.hostname }),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "uninstall_requested": true
    })))
}

#[utoipa::path(post, path = "/v1/nodes/{id}/docker-prune", tag = "nodes", responses((status = 200), (status = 404)))]
pub async fn docker_prune_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth.require(Permission::NodesWrite)?;
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    let node = nodes::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    if node.revoked_at.is_some() {
        return Err(ApiError(PlatformError::new(
            ErrorCode::NodeUntrusted,
            "This node has been revoked.",
        )));
    }
    nodes::request_docker_prune(&state.pool, id).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        Some(id),
        "nodes.docker_prune.requested",
        "node",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "hostname": node.hostname }),
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "ok": true, "docker_prune_requested": true }),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/nodes/enrollment-tokens",
    tag = "nodes",
    request_body = EnrollmentTokenRequest,
    responses((status = 200, body = EnrollmentTokenResponse))
)]
pub async fn create_enrollment_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<EnrollmentTokenRequest>,
) -> Result<Json<EnrollmentTokenResponse>, ApiError> {
    auth.require(Permission::NodesEnroll)?;
    let token = generate_token();
    let id = uuid::Uuid::now_v7().to_string();
    let expires_at = Utc::now() + Duration::seconds(state.config.enrollment_ttl_secs as i64);
    nodes::insert_enrollment_token(
        &state.pool,
        &id,
        &hash_token(&token),
        body.label.as_deref(),
        auth.user.id,
        expires_at,
    )
    .await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        None,
        "nodes.enrollment_token.created",
        "enrollment_token",
        Some(&id),
        None,
        None,
        serde_json::json!({ "label": body.label }),
    )
    .await?;
    Ok(Json(EnrollmentTokenResponse { token, expires_at }))
}

pub fn advertised_node_endpoint(state: &AppState) -> String {
    advertised_node_endpoint_from(
        state.config.allow_insecure_http,
        state.config.http_bind,
        state.config.node_bind,
        &state.config.public_url,
    )
}

pub(crate) fn advertised_node_endpoint_from(
    allow_insecure_http: bool,
    http_bind: SocketAddr,
    node_bind: SocketAddr,
    public_url: &str,
) -> String {
    if allow_insecure_http {
        let host = advertised_host(http_bind.ip(), public_url);
        format_endpoint("http", &host, http_bind.port())
    } else {
        let host = advertised_host(node_bind.ip(), public_url);
        format_endpoint("https", &host, node_bind.port())
    }
}

fn advertised_host(bind_ip: IpAddr, public_url: &str) -> String {
    if !bind_ip.is_unspecified() {
        return bind_ip.to_string();
    }
    url::Url::parse(public_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .filter(|h| !h.is_empty() && h != "0.0.0.0" && h != "::")
        .unwrap_or_else(|| "127.0.0.1".into())
}

fn format_endpoint(scheme: &str, host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("{scheme}://[{host}]:{port}")
    } else {
        format!("{scheme}://{host}:{port}")
    }
}

#[utoipa::path(
    post,
    path = "/v1/nodes/enroll",
    tag = "nodes",
    request_body = EnrollRequest,
    responses((status = 200, body = EnrollResponse), (status = 400))
)]
pub async fn enroll(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<EnrollRequest>,
) -> Result<Json<EnrollResponse>, ApiError> {
    if !protocol_compatible(body.protocol_version, PROTOCOL_VERSION) {
        return Err(ApiError(PlatformError::new(
            ErrorCode::ProtocolIncompatible,
            format!(
                "Agent protocol {} is not compatible with control plane protocol {PROTOCOL_VERSION}.",
                body.protocol_version
            ),
        )));
    }
    if body.hostname.trim().is_empty() {
        return Err(ApiError(
            PlatformError::validation("hostname is required").field("hostname"),
        ));
    }
    let token_hash = hash_token(&body.enrollment_token);
    let node_id = NodeId::new();
    let issued = state
        .ca
        .issue_node_cert(&node_id.to_string(), &body.hostname)
        .map_err(|e| {
            tracing::error!(error = %e, "failed to issue node certificate");
            ApiError(PlatformError::internal())
        })?;
    let node_token = generate_token();
    let name = body
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| body.hostname.clone());

    let mut tx = state.pool.begin().await?;
    let record = nodes::lock_enrollment_token(&mut tx, &token_hash)
        .await?
        .ok_or_else(|| {
            ApiError(PlatformError::new(
                ErrorCode::EnrollmentTokenInvalid,
                "Enrollment token is invalid or expired.",
            ))
        })?;
    if record.consumed_at.is_some() {
        return Err(ApiError(PlatformError::new(
            ErrorCode::EnrollmentTokenConsumed,
            "This enrollment token has already been used.",
        )));
    }
    if record.expires_at < Utc::now() {
        return Err(ApiError(PlatformError::new(
            ErrorCode::EnrollmentTokenInvalid,
            "Enrollment token is invalid or expired.",
        )));
    }
    nodes::insert_node_tx(
        &mut tx,
        &nodes::NewNode {
            id: node_id,
            name: &name,
            hostname: &body.hostname,
            agent_version: &body.agent_version,
            protocol_version: body.protocol_version,
            architecture: &body.architecture,
            os: &body.operating_system,
            labels: &body.labels,
            docker: &body.docker,
            fingerprint: &issued.fingerprint_sha256,
            token_hash: &hash_token(&node_token),
            resources: &body.resources,
        },
    )
    .await?;
    let consumed = nodes::consume_enrollment_token_tx(&mut tx, &record.id, node_id).await?;
    if consumed != 1 {
        return Err(ApiError(PlatformError::new(
            ErrorCode::EnrollmentTokenConsumed,
            "This enrollment token has already been used.",
        )));
    }
    tx.commit().await?;

    audit::record(
        &state.pool,
        None,
        Some(node_id),
        "nodes.enrolled",
        "node",
        Some(&node_id.to_string()),
        Some(&ip),
        None,
        serde_json::json!({
            "hostname": body.hostname,
            "architecture": body.architecture,
            "docker": format!("{:?}", body.docker.state),
        }),
    )
    .await?;
    Ok(Json(EnrollResponse {
        node_id,
        node_token,
        certificate_pem: issued.certificate_pem,
        private_key_pem: issued.private_key_pem,
        ca_pem: issued.ca_pem,
        heartbeat_interval_seconds: 15,
        protocol_version: PROTOCOL_VERSION,
        control_plane_version: fps_branding::VERSION.to_string(),
        node_endpoint: advertised_node_endpoint(&state),
    }))
}

#[utoipa::path(
    post,
    path = "/v1/nodes/{id}/heartbeat",
    tag = "nodes",
    request_body = HeartbeatRequest,
    responses((status = 200, body = HeartbeatResponse))
)]
pub async fn heartbeat(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, ApiError> {
    if !state.config.allow_insecure_http {
        return Err(ApiError(PlatformError::new(
            ErrorCode::NodeUntrusted,
            "Bearer heartbeats are disabled. Use the mTLS node endpoint.",
        )));
    }
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    let expected = nodes::find_token_hash(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            ApiError(PlatformError::new(
                ErrorCode::NodeUntrusted,
                "Node token is required.",
            ))
        })?;
    if !ct_eq_hex(&hash_token(presented), &expected) {
        return Err(ApiError(PlatformError::new(
            ErrorCode::NodeUntrusted,
            "Node identity was rejected.",
        )));
    }
    apply_heartbeat(&state, id, body).await
}

pub async fn heartbeat_mtls(
    State(state): State<AppState>,
    Path(id): Path<String>,
    axum::Extension(peer): axum::Extension<PeerFingerprint>,
    Json(body): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, ApiError> {
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    if peer.0.is_empty() {
        return Err(ApiError(PlatformError::new(
            ErrorCode::NodeUntrusted,
            "Client certificate is required.",
        )));
    }
    let bound = nodes::find_id_by_fingerprint(&state.pool, &peer.0)
        .await?
        .ok_or_else(|| {
            ApiError(PlatformError::new(
                ErrorCode::NodeUntrusted,
                "Node identity was rejected.",
            ))
        })?;
    if bound != id {
        return Err(ApiError(PlatformError::new(
            ErrorCode::NodeUntrusted,
            "Node identity was rejected.",
        )));
    }
    apply_heartbeat(&state, id, body).await
}

async fn apply_heartbeat(
    state: &AppState,
    id: NodeId,
    body: HeartbeatRequest,
) -> Result<Json<HeartbeatResponse>, ApiError> {
    if !protocol_compatible(body.protocol_version, PROTOCOL_VERSION) {
        return Err(ApiError(PlatformError::new(
            ErrorCode::ProtocolIncompatible,
            "Node protocol is incompatible.",
        )));
    }
    nodes::heartbeat(
        &state.pool,
        id,
        &body.agent_version,
        body.protocol_version,
        &body.docker,
        &body.resources,
        body.workload_count,
        body.note.as_deref(),
    )
    .await?;
    let _ = metrics::insert(
        &state.pool,
        id,
        None,
        body.resources.cpu_percent.map(f64::from),
        body.resources
            .memory_used_bytes
            .or(body.resources.memory_bytes)
            .map(|v| v as i64),
        body.resources.disk_available_bytes.map(|v| v as i64),
        body.resources.load_one,
        None,
    )
    .await;
    logs::append_chunks(&state.pool, id, &body.log_chunks).await?;
    for chunk in &body.log_chunks {
        state.log_hub.publish(crate::state::LogEvent {
            server_id: chunk.server_id,
            stream: chunk.stream.clone(),
            chunk: chunk.text.clone(),
            created_at: Utc::now(),
        });
    }
    for result in &body.job_results {
        apply_job_result(state, result).await?;
    }
    if let Some(ack) = &body.control_ack {
        if ack.uninstall.as_deref() == Some("completed") {
            nodes::mark_uninstalled(&state.pool, id).await?;
            let _ = nodes::revoke(&state.pool, id).await?;
            notifications::insert(
                &state.pool,
                "node",
                "Host uninstalled",
                "A game host finished uninstalling the agent and was revoked.",
            )
            .await?;
        }
        if ack.docker_prune.as_deref() == Some("completed") {
            nodes::clear_docker_prune(&state.pool, id).await?;
        }
    }
    for sample in &body.container_samples {
        let _ = metrics::insert(
            &state.pool,
            id,
            Some(sample.server_id),
            sample.cpu_percent.map(f64::from),
            sample.memory_bytes.map(|v| v as i64),
            None,
            None,
            Some(sample.running),
        )
        .await;
        if !sample.running {
            if let Some(server) = servers::get(&state.pool, sample.server_id).await? {
                // Installing / restoring often report not-running while the agent
                // unpacks files. Only a *running* server that disappeared is a crash.
                if matches!(server.summary.status, ServerStatus::Running) {
                    let failures = servers::record_crash(
                        &state.pool,
                        sample.server_id,
                        "Container stopped unexpectedly.",
                    )
                    .await?;
                    if failures < 3 {
                        if let Some(node_id) = server.summary.node_id {
                            jobs::enqueue(
                                &state.pool,
                                node_id,
                                Some(sample.server_id),
                                JobKind::Start,
                                serde_json::json!({
                                    "server_id": sample.server_id,
                                    "container_name": server.summary.container_name,
                                }),
                            )
                            .await?;
                            notifications::insert(
                                &state.pool,
                                "server",
                                "Crash restart",
                                &format!(
                                    "{} restarted after an unexpected stop (attempt {failures}/3).",
                                    server.summary.name
                                ),
                            )
                            .await?;
                        }
                    } else {
                        notifications::insert(
                            &state.pool,
                            "server",
                            "Crash loop",
                            &format!(
                                "{} stopped restarting after {failures} consecutive failures.",
                                server.summary.name
                            ),
                        )
                        .await?;
                    }
                }
            }
        } else {
            let _ = servers::clear_failures(&state.pool, sample.server_id).await;
        }
    }
    let node = nodes::get(&state.pool, id).await?;
    let settings = node
        .as_ref()
        .map(|n| NodeControlSettings {
            name: Some(n.name.clone()),
            labels: Some(n.labels.clone()),
            heartbeat_interval_seconds: Some(n.heartbeat_interval_seconds),
            maintenance: Some(n.maintenance),
            uninstall: n.uninstall_requested_at.is_some() && n.uninstalled_at.is_none(),
            docker_prune: n.docker_prune_requested,
        })
        .unwrap_or_default();
    let desired_drain = settings.maintenance.unwrap_or(false) || settings.uninstall;
    let skip_jobs = settings.uninstall || node.as_ref().is_some_and(|n| n.revoked_at.is_some());
    let claimed = if skip_jobs {
        Vec::new()
    } else {
        jobs::claim_for_node(&state.pool, id, 8).await?
    };
    Ok(Json(HeartbeatResponse {
        accepted: true,
        protocol_version: PROTOCOL_VERSION,
        server_time: Utc::now(),
        rotate_token: None,
        desired_drain,
        jobs: claimed,
        settings,
    }))
}

async fn apply_job_result(state: &AppState, result: &JobResult) -> Result<(), ApiError> {
    let Some(job) = jobs::complete(&state.pool, result).await? else {
        return Ok(());
    };
    let Some(server_id) = job.server_id else {
        return Ok(());
    };
    match job.kind {
        JobKind::Install | JobKind::Start | JobKind::Restore => {
            if result.success {
                servers::set_container(
                    &state.pool,
                    server_id,
                    result.container_id.as_deref(),
                    ServerStatus::Running,
                )
                .await?;
            } else {
                servers::set_status(
                    &state.pool,
                    server_id,
                    ServerStatus::Failed,
                    Some(&result.message),
                )
                .await?;
                notifications::insert(
                    &state.pool,
                    "job",
                    "Job failed",
                    &format!("{} failed: {}", job.kind.as_str(), result.message),
                )
                .await?;
            }
        }
        JobKind::Stop => {
            if result.success {
                servers::set_status(&state.pool, server_id, ServerStatus::Stopped, None).await?;
            } else {
                servers::set_status(
                    &state.pool,
                    server_id,
                    ServerStatus::Failed,
                    Some(&result.message),
                )
                .await?;
            }
        }
        JobKind::Backup => {
            if let Some(backup_id) = job
                .payload
                .get("backup_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<BackupId>().ok())
            {
                backups::complete(
                    &state.pool,
                    backup_id,
                    result.success,
                    result.backup_path.as_deref(),
                    result.backup_bytes.map(|b| b as i64),
                    if result.success {
                        None
                    } else {
                        Some(result.message.as_str())
                    },
                )
                .await?;
            }
        }
        JobKind::FilesList => {
            if let Some(files) = &result.files {
                servers::set_files(&state.pool, server_id, files).await?;
            }
        }
        JobKind::FilesRead => {
            if let Some(content) = &result.file_content {
                let path = job
                    .payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                servers::set_last_file(
                    &state.pool,
                    server_id,
                    &serde_json::json!({
                        "path": path,
                        "content": content,
                        "updated_at": Utc::now(),
                    }),
                )
                .await?;
            }
        }
        JobKind::FilesWrite | JobKind::Exec => {}
        JobKind::AddonInstall => {
            apply_addon_result(state, &job, result, true).await?;
        }
        JobKind::AddonUninstall => {
            apply_addon_result(state, &job, result, false).await?;
        }
    }
    Ok(())
}

async fn apply_addon_result(
    state: &AppState,
    job: &crate::db::jobs::JobRecord,
    result: &JobResult,
    install: bool,
) -> Result<(), ApiError> {
    let Some(server_id) = job.server_id else {
        return Ok(());
    };
    let slug = job
        .payload
        .get("addon_slug")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(row) = crate::db::addons::get_for_server(&state.pool, server_id, slug).await? else {
        return Ok(());
    };
    if result.success {
        if install {
            let tracked = result
                .tracked_paths
                .clone()
                .unwrap_or_else(|| row.summary.tracked_paths.clone());
            crate::db::addons::mark_installed(&state.pool, row.summary.id, &tracked).await?;
        } else {
            crate::db::addons::delete(&state.pool, row.summary.id).await?;
        }
    } else {
        crate::db::addons::mark_failed(&state.pool, row.summary.id, &result.message).await?;
        notifications::insert(
            &state.pool,
            "addon",
            if install {
                "Addon install failed"
            } else {
                "Addon uninstall failed"
            },
            &result.message,
        )
        .await?;
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/v1/nodes/{id}/revoke",
    tag = "nodes",
    responses((status = 200), (status = 404))
)]
pub async fn revoke_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth.require(Permission::NodesWrite)?;
    let id: NodeId = id
        .parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid node id")))?;
    let node = nodes::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::not_found("node")))?;
    if node.revoked_at.is_some() {
        return Ok(Json(
            serde_json::json!({ "ok": true, "already_revoked": true }),
        ));
    }
    nodes::revoke(&state.pool, id).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        Some(id),
        "nodes.revoked",
        "node",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "hostname": node.hostname }),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod advertised_endpoint_tests {
    use super::*;

    #[test]
    fn insecure_http_uses_public_host_when_bound_to_unspecified() {
        let endpoint = advertised_node_endpoint_from(
            true,
            "0.0.0.0:47890".parse().unwrap(),
            "0.0.0.0:47891".parse().unwrap(),
            "http://10.0.0.8:47890",
        );
        assert_eq!(endpoint, "http://10.0.0.8:47890");
    }

    #[test]
    fn insecure_http_keeps_loopback_bind_for_tests() {
        let endpoint = advertised_node_endpoint_from(
            true,
            "127.0.0.1:51234".parse().unwrap(),
            "127.0.0.1:51235".parse().unwrap(),
            "http://127.0.0.1:47890",
        );
        assert_eq!(endpoint, "http://127.0.0.1:51234");
    }

    #[test]
    fn mtls_uses_public_host_and_node_port() {
        let endpoint = advertised_node_endpoint_from(
            false,
            "0.0.0.0:47890".parse().unwrap(),
            "0.0.0.0:47891".parse().unwrap(),
            "https://panel.example.test:47890",
        );
        assert_eq!(endpoint, "https://panel.example.test:47891");
    }
}
