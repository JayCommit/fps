use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use fps_domain::{AuditEventId, NodeId, Permission, UserId};
use serde::Serialize;
use utoipa::ToSchema;

use crate::db::audit;
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEventView {
    pub id: AuditEventId,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub actor_user_id: Option<UserId>,
    pub actor_node_id: Option<NodeId>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[utoipa::path(get, path = "/v1/audit", tag = "identity", responses((status = 200, body = [AuditEventView])))]
pub async fn list_audit(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<AuditEventView>>, ApiError> {
    auth.require(Permission::AuditRead)?;
    let rows = audit::list(&state.pool, 100).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| AuditEventView {
                id: r.id,
                action: r.action,
                resource_type: r.resource_type,
                resource_id: r.resource_id,
                actor_user_id: r.actor_user_id,
                actor_node_id: r.actor_node_id,
                details: r.details,
                created_at: r.created_at,
            })
            .collect(),
    ))
}
