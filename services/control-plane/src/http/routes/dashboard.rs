use axum::extract::State;
use axum::Json;
use fps_domain::{NodeStatus, Permission};
use serde::Serialize;
use utoipa::ToSchema;

use crate::db::{nodes, servers, settings};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardSummary {
    pub product: String,
    pub version: String,
    pub setup_completed: bool,
    pub nodes_total: usize,
    pub nodes_online: usize,
    pub nodes_degraded: usize,
    pub nodes_offline: usize,
    pub docker_available: usize,
    pub servers_total: i64,
    pub servers_running: i64,
    pub alerts: Vec<DashboardAlert>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardAlert {
    pub severity: String,
    pub title: String,
    pub detail: String,
}

#[utoipa::path(get, path = "/v1/dashboard", tag = "dashboard", responses((status = 200, body = DashboardSummary)))]
pub async fn summary(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<DashboardSummary>, ApiError> {
    auth.require(Permission::NodesRead)?;
    let records = nodes::list(&state.pool).await?;
    let timeout = state.config.heartbeat_timeout_secs;
    let health: Vec<_> = records.iter().map(|n| n.health(timeout)).collect();
    let nodes_online = health
        .iter()
        .filter(|h| h.status == NodeStatus::Online)
        .count();
    let nodes_degraded = health
        .iter()
        .filter(|h| h.status == NodeStatus::Degraded)
        .count();
    let nodes_offline = health
        .iter()
        .filter(|h| matches!(h.status, NodeStatus::Offline | NodeStatus::Enrolling))
        .count();
    let docker_available = health
        .iter()
        .filter(|h| h.docker == fps_domain::DockerState::Available)
        .count();
    let mut alerts = Vec::new();
    if records.is_empty() {
        alerts.push(DashboardAlert {
            severity: "info".into(),
            title: "No game nodes enrolled".into(),
            detail: "Create an enrollment token and run the node agent to connect Homer (or a local test node).".into(),
        });
    }
    for h in &health {
        if h.status == NodeStatus::Offline {
            alerts.push(DashboardAlert {
                severity: "critical".into(),
                title: "Node offline".into(),
                detail: format!("Node {} has not sent a heartbeat.", h.id),
            });
        }
    }
    let (servers_total, servers_running) = servers::counts(&state.pool).await?;
    Ok(Json(DashboardSummary {
        product: fps_branding::DISPLAY_NAME.to_string(),
        version: fps_branding::VERSION.to_string(),
        setup_completed: settings::setup_completed(&state.pool).await?,
        nodes_total: records.len(),
        nodes_online,
        nodes_degraded,
        nodes_offline,
        docker_available,
        servers_total,
        servers_running,
        alerts,
    }))
}
