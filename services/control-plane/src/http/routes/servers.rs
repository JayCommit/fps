use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use fps_domain::{
    BackupId, JobKind, Permission, ServerId, ServerStatus, ServerSummary, TemplateId,
};
use fps_templates::interpolate_map;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

use crate::db::{
    allocations, audit, backups, jobs, logs, nodes, notifications, schedules, servers, templates,
};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateServerRequest {
    pub name: String,
    pub template_id: TemplateId,
    pub environment: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerDetail {
    #[serde(flatten)]
    pub summary: ServerSummary,
    pub environment: serde_json::Value,
    pub files: Option<serde_json::Value>,
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
    Ok(Json(rows.into_iter().map(|r| r.summary).collect()))
}

#[utoipa::path(get, path = "/v1/servers/{id}", tag = "servers", responses((status = 200, body = ServerDetail)))]
pub async fn get_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ServerDetail>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rec = load_server(&state, &id).await?;
    Ok(Json(detail(&rec)))
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
    let protocol = template
        .summary
        .ports
        .first()
        .map(|p| p.protocol.as_str())
        .unwrap_or("tcp");
    let alloc = allocations::allocate_next(&state.pool, node.id, protocol).await?;
    let mut env: BTreeMap<String, String> =
        serde_json::from_str(&template.env_json).unwrap_or_default();
    if let Some(overrides) = body.environment {
        env.extend(overrides);
    }
    env.insert("SERVER_PORT".into(), alloc.port.to_string());
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
        alloc.id,
        &serde_json::to_string(&env).unwrap_or_else(|_| "{}".into()),
        template.summary.memory_mb,
        template.cpu_shares,
        &container_name,
        auth.user.id,
    )
    .await?;
    allocations::assign_server(&state.pool, alloc.id, server_id).await?;
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
    let ports: Vec<serde_json::Value> = template
        .summary
        .ports
        .iter()
        .map(|p| {
            serde_json::json!({
                "host": alloc.port,
                "container": p.container_port,
                "protocol": p.protocol,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "server_id": server_id,
        "name": name,
        "image": template.summary.docker_image,
        "env": env,
        "cmd": cmd,
        "ports": ports,
        "memory_mb": template.summary.memory_mb,
        "volume_path": template.volume_path,
        "container_name": container_name,
    });
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

fn detail(rec: &servers::ServerRecord) -> ServerDetail {
    ServerDetail {
        summary: rec.summary.clone(),
        environment: serde_json::from_str(&rec.environment_json).unwrap_or(serde_json::json!({})),
        files: rec
            .files_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        container_id: rec.container_id.clone(),
    }
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
