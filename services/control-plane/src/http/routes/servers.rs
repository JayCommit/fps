use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use fps_domain::{
    AllocatedPort, BackupId, BackupStatus, JobId, JobKind, Permission, ServerId, ServerStatus,
    ServerSummary, TemplateId,
};
use fps_templates::interpolate_map;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

use crate::db::{
    allocations, audit, backups, jobs, logs, metrics, nodes, notifications, schedules, servers,
    templates,
};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::{AppState, LogEvent};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateServerRequest {
    pub name: String,
    pub template_id: TemplateId,
    pub environment: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchServerRequest {
    pub name: Option<String>,
    pub environment: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerDetail {
    #[serde(flatten)]
    pub summary: ServerSummary,
    pub environment: serde_json::Value,
    pub files: Option<serde_json::Value>,
    pub last_file: Option<serde_json::Value>,
    pub container_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogLine {
    pub stream: String,
    pub chunk: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ServerIdQuery {
    pub server_id: Option<ServerId>,
}

#[utoipa::path(get, path = "/v1/servers", tag = "servers", responses((status = 200, body = [ServerSummary])))]
pub async fn list_servers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<ServerSummary>>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rows = servers::list(&state.pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for rec in rows {
        let mut summary = rec.summary;
        summary.ports = allocated_ports(&state, summary.id).await?;
        out.push(summary);
    }
    Ok(Json(out))
}

#[utoipa::path(get, path = "/v1/servers/{id}", tag = "servers", responses((status = 200, body = ServerDetail)))]
pub async fn get_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ServerDetail>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rec = load_server(&state, &id).await?;
    let ports = allocated_ports(&state, rec.summary.id).await?;
    Ok(Json(detail(&rec, ports)))
}

#[utoipa::path(
    post,
    path = "/v1/servers",
    tag = "servers",
    request_body = CreateServerRequest,
    responses((status = 200, body = ServerSummary))
)]
pub async fn create_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateServerRequest>,
) -> Result<Json<ServerSummary>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError(
            fps_domain::PlatformError::validation("Server name is required.").field("name"),
        ));
    }
    templates::ensure_catalogue(&state.pool).await?;
    let template = templates::get(&state.pool, body.template_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("template")))?;
    let node = nodes::pick_schedulable(&state.pool).await?.ok_or_else(|| {
        ApiError(fps_domain::PlatformError::validation(
            "No online node with Docker available. Enroll an agent first.",
        ))
    })?;
    let bindings = allocate_or_busy(&state.pool, node.id, &template.summary.ports).await?;
    let primary = bindings
        .first()
        .ok_or_else(|| {
            ApiError(fps_domain::PlatformError::validation(
                "Template has no ports to publish.",
            ))
        })?
        .allocation_id;
    let mut env: BTreeMap<String, String> =
        serde_json::from_str(&template.env_json).unwrap_or_default();
    if let Some(overrides) = body.environment {
        env.extend(overrides);
    }
    env.insert("SERVER_NAME".into(), name.to_string());
    let env = interpolate_map(&env, &env);
    let server_id = ServerId::new();
    let container_name = format!("fps-{}", &server_id.to_string()[..8]);
    servers::insert(
        &state.pool,
        server_id,
        name,
        template.summary.id,
        node.id,
        primary,
        &serde_json::to_string(&env).unwrap_or_else(|_| "{}".into()),
        template.summary.memory_mb,
        template.cpu_shares,
        &container_name,
        auth.user.id,
    )
    .await?;
    allocations::assign_all(
        &state.pool,
        server_id,
        &bindings.iter().map(|b| b.allocation_id).collect::<Vec<_>>(),
    )
    .await?;
    let mut cmd: Vec<String> = Vec::new();
    if template.summary.slug == "http-echo" {
        let text = env
            .get("ECHO_TEXT")
            .cloned()
            .unwrap_or_else(|| "fps".into());
        cmd = vec!["-listen=:5678".into(), format!("-text={text}")];
    } else if let Some(startup) = &template.startup_command {
        cmd = vec!["sh".into(), "-c".into(), startup.clone()];
    }
    let payload = install_payload(
        server_id,
        name,
        &template.summary.docker_image,
        &env,
        &cmd,
        &bindings,
        template.summary.memory_mb,
        &template.volume_path,
        &container_name,
        false,
        0,
    );
    jobs::enqueue(
        &state.pool,
        node.id,
        Some(server_id),
        JobKind::Install,
        payload,
    )
    .await?;
    servers::set_status(&state.pool, server_id, ServerStatus::Installing, None).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        Some(node.id),
        "servers.created",
        "server",
        Some(&server_id.to_string()),
        None,
        None,
        serde_json::json!({ "name": name, "node_id": node.id }),
    )
    .await?;
    notifications::insert(
        &state.pool,
        "server",
        "Server queued",
        &format!("{name} is installing on node {}.", node.name),
    )
    .await?;
    let rec = servers::get(&state.pool, server_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::internal()))?;
    let mut summary = rec.summary;
    summary.ports = allocated_ports(&state, server_id).await?;
    Ok(Json(summary))
}

#[utoipa::path(
    patch,
    path = "/v1/servers/{id}",
    tag = "servers",
    request_body = PatchServerRequest,
    responses((status = 200, body = ServerDetail))
)]
pub async fn patch_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PatchServerRequest>,
) -> Result<Json<ServerDetail>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let rec = load_server(&state, &id).await?;
    if matches!(rec.summary.status, ServerStatus::Deleting) {
        return Err(ApiError(fps_domain::PlatformError::new(
            fps_domain::ErrorCode::Conflict,
            "This server is being deleted.",
        )));
    }
    let name = match body.name.as_deref().map(str::trim) {
        Some("") => {
            return Err(ApiError(
                fps_domain::PlatformError::validation("Server name is required.").field("name"),
            ));
        }
        Some(n) => Some(n.to_string()),
        None => None,
    };
    let mut env: BTreeMap<String, String> =
        serde_json::from_str(&rec.environment_json).unwrap_or_default();
    let env_changed = body.environment.is_some();
    if let Some(overrides) = body.environment {
        env.extend(overrides);
    }
    if let Some(n) = &name {
        env.insert("SERVER_NAME".into(), n.clone());
    }
    let env = interpolate_map(&env, &env);
    let env_json = serde_json::to_string(&env).unwrap_or_else(|_| "{}".into());
    servers::update_name_and_env(
        &state.pool,
        rec.summary.id,
        name.as_deref(),
        if env_changed || name.is_some() {
            Some(&env_json)
        } else {
            None
        },
    )
    .await?;
    let rec = load_server(&state, &id).await?;
    if env_changed {
        if let Some(node_id) = rec.summary.node_id {
            let template = templates::get(&state.pool, rec.summary.template_id)
                .await?
                .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("template")))?;
            let bindings =
                allocations::list_bindings_for_server(&state.pool, rec.summary.id).await?;
            let mut cmd: Vec<String> = Vec::new();
            if template.summary.slug == "http-echo" {
                let text = env
                    .get("ECHO_TEXT")
                    .cloned()
                    .unwrap_or_else(|| "fps".into());
                cmd = vec!["-listen=:5678".into(), format!("-text={text}")];
            } else if let Some(startup) = &template.startup_command {
                cmd = vec!["sh".into(), "-c".into(), startup.clone()];
            }
            let payload = install_payload(
                rec.summary.id,
                &rec.summary.name,
                &template.summary.docker_image,
                &env,
                &cmd,
                &bindings,
                rec.summary.memory_mb,
                &template.volume_path,
                rec.summary.container_name.as_deref().unwrap_or(""),
                true,
                0,
            );
            jobs::enqueue(
                &state.pool,
                node_id,
                Some(rec.summary.id),
                JobKind::Install,
                payload,
            )
            .await?;
            servers::set_status(&state.pool, rec.summary.id, ServerStatus::Installing, None)
                .await?;
        }
    }
    audit::record(
        &state.pool,
        Some(auth.user.id),
        rec.summary.node_id,
        "servers.updated",
        "server",
        Some(&rec.summary.id.to_string()),
        None,
        None,
        serde_json::json!({ "name": rec.summary.name }),
    )
    .await?;
    let rec = load_server(&state, &id).await?;
    let ports = allocated_ports(&state, rec.summary.id).await?;
    Ok(Json(detail(&rec, ports)))
}

#[utoipa::path(
    delete,
    path = "/v1/servers/{id}",
    tag = "servers",
    responses((status = 200, body = ServerSummary), (status = 204))
)]
pub async fn delete_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ServerSummary>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let rec = load_server(&state, &id).await?;
    let node_id = rec.summary.node_id;
    let container_name = rec.summary.container_name.clone().unwrap_or_default();
    servers::set_status(&state.pool, rec.summary.id, ServerStatus::Deleting, None).await?;
    if let Some(node_id) = node_id {
        jobs::enqueue(
            &state.pool,
            node_id,
            Some(rec.summary.id),
            JobKind::Delete,
            serde_json::json!({
                "server_id": rec.summary.id,
                "container_name": container_name,
            }),
        )
        .await?;
        audit::record(
            &state.pool,
            Some(auth.user.id),
            Some(node_id),
            "servers.delete_requested",
            "server",
            Some(&rec.summary.id.to_string()),
            None,
            None,
            serde_json::json!({ "name": rec.summary.name }),
        )
        .await?;
        notifications::insert(
            &state.pool,
            "server",
            "Server deleting",
            &format!("{} is being removed from the node.", rec.summary.name),
        )
        .await?;
        let rec = load_server(&state, &id).await?;
        return Ok(Json(rec.summary));
    }
    let name = rec.summary.name.clone();
    let sid = rec.summary.id;
    servers::purge(&state.pool, sid).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        None,
        "servers.deleted",
        "server",
        Some(&sid.to_string()),
        None,
        None,
        serde_json::json!({ "name": name }),
    )
    .await?;
    Ok(Json(rec.summary))
}

#[utoipa::path(post, path = "/v1/servers/{id}/start", tag = "servers", responses((status = 200, body = ServerSummary)))]
pub async fn start_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ServerSummary>, ApiError> {
    enqueue_lifecycle(&state, &auth, &id, JobKind::Start).await
}

#[utoipa::path(post, path = "/v1/servers/{id}/stop", tag = "servers", responses((status = 200, body = ServerSummary)))]
pub async fn stop_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ServerSummary>, ApiError> {
    enqueue_lifecycle(&state, &auth, &id, JobKind::Stop).await
}

#[utoipa::path(post, path = "/v1/servers/{id}/backup", tag = "servers", responses((status = 200)))]
pub async fn backup_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    auth.require(Permission::BackupsWrite)?;
    let rec = load_server(&state, &id).await?;
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    let backup_id = BackupId::new();
    backups::insert_pending(&state.pool, backup_id, rec.summary.id, node_id).await?;
    jobs::enqueue(
        &state.pool,
        node_id,
        Some(rec.summary.id),
        JobKind::Backup,
        serde_json::json!({
            "server_id": rec.summary.id,
            "container_name": rec.summary.container_name,
            "backup_id": backup_id,
        }),
    )
    .await?;
    Ok(StatusCode::OK)
}

#[utoipa::path(get, path = "/v1/servers/{id}/logs", tag = "servers", responses((status = 200, body = [LogLine])))]
pub async fn server_logs(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<LogLine>>, ApiError> {
    auth.require(Permission::ServersConsole)?;
    let rec = load_server(&state, &id).await?;
    let rows = logs::recent(&state.pool, rec.summary.id, 200).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| LogLine {
                stream: r.stream,
                chunk: r.chunk,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

#[utoipa::path(get, path = "/v1/servers/{id}/files", tag = "servers", responses((status = 200)))]
pub async fn server_files(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rec = load_server(&state, &id).await?;
    let files = rec
        .files_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::json!([]));
    Ok(Json(serde_json::json!({ "files": files })))
}

#[utoipa::path(post, path = "/v1/servers/{id}/files/refresh", tag = "servers", responses((status = 200)))]
pub async fn refresh_files(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let rec = load_server(&state, &id).await?;
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    jobs::enqueue(
        &state.pool,
        node_id,
        Some(rec.summary.id),
        JobKind::FilesList,
        serde_json::json!({
            "server_id": rec.summary.id,
            "container_name": rec.summary.container_name,
        }),
    )
    .await?;
    Ok(StatusCode::OK)
}

#[utoipa::path(get, path = "/v1/backups", tag = "backups", responses((status = 200)))]
pub async fn list_backups(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ServerIdQuery>,
) -> Result<Json<Vec<fps_domain::BackupSummary>>, ApiError> {
    auth.require(Permission::BackupsRead)?;
    Ok(Json(backups::list(&state.pool, q.server_id).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateScheduleRequest {
    pub server_id: ServerId,
    pub name: String,
    pub interval_seconds: i32,
    pub action: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScheduleView {
    pub id: fps_domain::ScheduleId,
    pub server_id: ServerId,
    pub name: String,
    pub interval_seconds: i32,
    pub action: String,
    pub enabled: bool,
    pub next_run_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/v1/schedules", tag = "servers", responses((status = 200)))]
pub async fn list_schedules(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ServerIdQuery>,
) -> Result<Json<Vec<ScheduleView>>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rows = schedules::list(&state.pool, q.server_id).await?;
    Ok(Json(rows.into_iter().map(schedule_view).collect()))
}

#[utoipa::path(post, path = "/v1/schedules", tag = "servers", request_body = CreateScheduleRequest, responses((status = 200)))]
pub async fn create_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateScheduleRequest>,
) -> Result<Json<ScheduleView>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    if !matches!(body.action.as_str(), "start" | "stop" | "backup") {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "action must be start, stop, or backup",
        )));
    }
    if body.interval_seconds < 30 {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "interval_seconds must be at least 30",
        )));
    }
    let _ = load_server(&state, &body.server_id.to_string()).await?;
    let id = schedules::insert(
        &state.pool,
        body.server_id,
        &body.name,
        body.interval_seconds,
        &body.action,
    )
    .await?;
    let rows = schedules::list(&state.pool, Some(body.server_id)).await?;
    let rec = rows
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError(fps_domain::PlatformError::internal()))?;
    Ok(Json(schedule_view(rec)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchScheduleRequest {
    pub enabled: Option<bool>,
}

#[utoipa::path(patch, path = "/v1/schedules/{id}", tag = "servers", responses((status = 200)))]
pub async fn patch_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PatchScheduleRequest>,
) -> Result<StatusCode, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let id: fps_domain::ScheduleId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid schedule id")))?;
    if let Some(enabled) = body.enabled {
        schedules::set_enabled(&state.pool, id, enabled).await?;
    }
    Ok(StatusCode::OK)
}

async fn enqueue_lifecycle(
    state: &AppState,
    auth: &AuthUser,
    id: &str,
    kind: JobKind,
) -> Result<Json<ServerSummary>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let rec = load_server(state, id).await?;
    if matches!(rec.summary.status, ServerStatus::Deleting) {
        return Err(ApiError(fps_domain::PlatformError::new(
            fps_domain::ErrorCode::Conflict,
            "This server is being deleted.",
        )));
    }
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    jobs::enqueue(
        &state.pool,
        node_id,
        Some(rec.summary.id),
        kind,
        serde_json::json!({
            "server_id": rec.summary.id,
            "container_name": rec.summary.container_name,
        }),
    )
    .await?;
    Ok(Json(rec.summary))
}

async fn load_server(state: &AppState, id: &str) -> Result<servers::ServerRecord, ApiError> {
    let id: ServerId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid server id")))?;
    servers::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("server")))
}

fn detail(rec: &servers::ServerRecord, ports: Vec<AllocatedPort>) -> ServerDetail {
    let mut summary = rec.summary.clone();
    summary.ports = ports;
    ServerDetail {
        summary,
        environment: serde_json::from_str(&rec.environment_json).unwrap_or(serde_json::json!({})),
        files: rec
            .files_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        last_file: rec
            .last_file_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        container_id: rec.container_id.clone(),
    }
}

async fn allocated_ports(
    state: &AppState,
    server_id: ServerId,
) -> Result<Vec<AllocatedPort>, ApiError> {
    let bindings = allocations::list_bindings_for_server(&state.pool, server_id).await?;
    Ok(bindings.into_iter().map(to_allocated_port).collect())
}

fn to_allocated_port(bind: allocations::AllocatedBinding) -> AllocatedPort {
    AllocatedPort {
        name: bind.name,
        protocol: bind.protocol,
        container_port: bind.container_port as u16,
        host_port: bind.host_port as u16,
        ip: bind.ip,
    }
}

async fn allocate_or_busy(
    pool: &sqlx::MySqlPool,
    node_id: fps_domain::NodeId,
    ports: &[fps_domain::PortMapping],
) -> Result<Vec<allocations::AllocatedBinding>, ApiError> {
    match allocations::allocate_for_ports(pool, node_id, ports).await {
        Ok(bindings) => Ok(bindings),
        Err(sqlx::Error::Protocol(msg)) => Err(ApiError(fps_domain::PlatformError::validation(
            msg.to_string(),
        ))),
        Err(err) => Err(err.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn install_payload(
    server_id: ServerId,
    name: &str,
    image: &str,
    env: &BTreeMap<String, String>,
    cmd: &[String],
    bindings: &[allocations::AllocatedBinding],
    memory_mb: i32,
    volume_path: &str,
    container_name: &str,
    replace: bool,
    port_retries: u32,
) -> serde_json::Value {
    let ports: Vec<serde_json::Value> = bindings
        .iter()
        .map(|b| {
            serde_json::json!({
                "host": b.host_port,
                "container": b.container_port,
                "protocol": b.protocol,
            })
        })
        .collect();
    serde_json::json!({
        "server_id": server_id,
        "name": name,
        "image": image,
        "env": env,
        "cmd": cmd,
        "ports": ports,
        "memory_mb": memory_mb,
        "volume_path": volume_path,
        "container_name": container_name,
        "replace": replace,
        "port_retries": port_retries,
    })
}

fn schedule_view(rec: crate::db::schedules::ScheduleRecord) -> ScheduleView {
    ScheduleView {
        id: rec.id,
        server_id: rec.server_id,
        name: rec.name,
        interval_seconds: rec.interval_seconds,
        action: rec.action,
        enabled: rec.enabled,
        next_run_at: rec.next_run_at,
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FileBody {
    pub path: String,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExecBody {
    pub command: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JobView {
    pub id: JobId,
    pub kind: String,
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetricPoint {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<i64>,
    pub disk_available_bytes: Option<i64>,
    pub load_one: Option<f32>,
    pub running: Option<bool>,
}

#[utoipa::path(post, path = "/v1/backups/{id}/restore", tag = "backups", responses((status = 200)))]
pub async fn restore_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    auth.require(Permission::BackupsWrite)?;
    let backup_id: BackupId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid backup id")))?;
    let backup = backups::get(&state.pool, backup_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("backup")))?;
    if backup.status != BackupStatus::Succeeded {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "Only a succeeded backup can be restored.",
        )));
    }
    let rec = servers::get(&state.pool, backup.server_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("server")))?;
    jobs::enqueue(
        &state.pool,
        backup.node_id,
        Some(backup.server_id),
        JobKind::Restore,
        serde_json::json!({
            "server_id": backup.server_id,
            "container_name": rec.summary.container_name,
            "backup_id": backup.id,
            "archive_path": backup.archive_path,
        }),
    )
    .await?;
    servers::set_status(
        &state.pool,
        backup.server_id,
        ServerStatus::Installing,
        None,
    )
    .await?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/v1/servers/{id}/files/read", tag = "servers", responses((status = 200)))]
pub async fn read_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<FileBody>,
) -> Result<Json<JobView>, ApiError> {
    auth.require(Permission::ServersRead)?;
    enqueue_file_job(&state, &id, JobKind::FilesRead, &body.path, None).await
}

#[utoipa::path(post, path = "/v1/servers/{id}/files/write", tag = "servers", responses((status = 200)))]
pub async fn write_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<FileBody>,
) -> Result<Json<JobView>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    enqueue_file_job(
        &state,
        &id,
        JobKind::FilesWrite,
        &body.path,
        body.content.as_deref(),
    )
    .await
}

#[utoipa::path(post, path = "/v1/servers/{id}/exec", tag = "servers", responses((status = 200)))]
pub async fn exec_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ExecBody>,
) -> Result<Json<JobView>, ApiError> {
    auth.require(Permission::ServersConsole)?;
    if body.command.trim().is_empty() {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "command is required",
        )));
    }
    let rec = load_server(&state, &id).await?;
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    let job_id = jobs::enqueue(
        &state.pool,
        node_id,
        Some(rec.summary.id),
        JobKind::Exec,
        serde_json::json!({
            "server_id": rec.summary.id,
            "container_name": rec.summary.container_name,
            "command": body.command,
        }),
    )
    .await?;
    Ok(Json(JobView {
        id: job_id,
        kind: JobKind::Exec.as_str().into(),
        status: "queued".into(),
        result: None,
        created_at: chrono::Utc::now(),
    }))
}

#[utoipa::path(get, path = "/v1/jobs/{id}", tag = "servers", responses((status = 200)))]
pub async fn get_job(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<JobView>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let id: JobId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid job id")))?;
    let job = jobs::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("job")))?;
    Ok(Json(JobView {
        id: job.id,
        kind: job.kind.as_str().into(),
        status: job.status.as_str().into(),
        result: job.result,
        created_at: job.created_at,
    }))
}

#[utoipa::path(get, path = "/v1/servers/{id}/metrics", tag = "servers", responses((status = 200)))]
pub async fn server_metrics(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<MetricPoint>>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rec = load_server(&state, &id).await?;
    let rows = metrics::list_for_server(&state.pool, rec.summary.id, 120).await?;
    Ok(Json(rows.into_iter().map(metric_point).collect()))
}

#[utoipa::path(get, path = "/v1/nodes/{id}/metrics", tag = "nodes", responses((status = 200)))]
pub async fn node_metrics(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<MetricPoint>>, ApiError> {
    auth.require(Permission::NodesRead)?;
    let id: fps_domain::NodeId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid node id")))?;
    let rows = metrics::list_for_node(&state.pool, id, 120).await?;
    Ok(Json(rows.into_iter().map(metric_point).collect()))
}

pub async fn server_console(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    auth.require(Permission::ServersConsole)?;
    let rec = load_server(&state, &id).await?;
    let server_id = rec.summary.id;
    let history = logs::recent(&state.pool, server_id, 200).await?;
    Ok(ws.on_upgrade(move |socket| console_socket(socket, state, server_id, history)))
}

async fn console_socket(
    mut socket: WebSocket,
    state: AppState,
    server_id: ServerId,
    history: Vec<logs::LogRecord>,
) {
    for line in history {
        let payload = serde_json::json!({
            "type": "log",
            "stream": line.stream,
            "chunk": line.chunk,
            "created_at": line.created_at,
        });
        if socket
            .send(Message::Text(payload.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }
    let mut rx = state.log_hub.subscribe();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            match value.get("type").and_then(|v| v.as_str()) {
                                Some("stdin") | Some("exec") => {
                                    if let Some(command) = value.get("data").or_else(|| value.get("command")).and_then(|v| v.as_str()) {
                                        if let Ok(Some(rec)) = servers::get(&state.pool, server_id).await {
                                            if let Some(node_id) = rec.summary.node_id {
                                                let _ = jobs::enqueue(
                                                    &state.pool,
                                                    node_id,
                                                    Some(server_id),
                                                    JobKind::Exec,
                                                    serde_json::json!({
                                                        "server_id": server_id,
                                                        "container_name": rec.summary.container_name,
                                                        "command": command,
                                                    }),
                                                ).await;
                                            }
                                        }
                                    }
                                }
                                Some("ping") => {
                                    let _ = socket.send(Message::Text("{\"type\":\"pong\"}".into())).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            event = rx.recv() => {
                match event {
                    Ok(LogEvent { server_id: sid, stream, chunk, created_at }) if sid == server_id => {
                        let payload = serde_json::json!({
                            "type": "log",
                            "stream": stream,
                            "chunk": chunk,
                            "created_at": created_at,
                        });
                        if socket.send(Message::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        }
    }
}

async fn enqueue_file_job(
    state: &AppState,
    id: &str,
    kind: JobKind,
    path: &str,
    content: Option<&str>,
) -> Result<Json<JobView>, ApiError> {
    if path.trim().is_empty() {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "path is required",
        )));
    }
    let rec = load_server(state, id).await?;
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    let job_id = jobs::enqueue(
        &state.pool,
        node_id,
        Some(rec.summary.id),
        kind,
        serde_json::json!({
            "server_id": rec.summary.id,
            "container_name": rec.summary.container_name,
            "path": path,
            "content": content,
        }),
    )
    .await?;
    Ok(Json(JobView {
        id: job_id,
        kind: kind.as_str().into(),
        status: "queued".into(),
        result: None,
        created_at: chrono::Utc::now(),
    }))
}

fn metric_point(sample: crate::db::metrics::Sample) -> MetricPoint {
    MetricPoint {
        created_at: sample.created_at,
        cpu_percent: sample.cpu_percent,
        memory_bytes: sample.memory_bytes,
        disk_available_bytes: sample.disk_available_bytes,
        load_one: sample.load_one,
        running: sample.running,
    }
}
