use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use fps_domain::{NotificationId, Permission};
use serde::Serialize;
use utoipa::ToSchema;

use crate::db::notifications;
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationView {
    pub id: NotificationId,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[utoipa::path(get, path = "/v1/notifications", tag = "ops", responses((status = 200, body = [NotificationView])))]
pub async fn list_notifications(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<NotificationView>>, ApiError> {
    auth.require(Permission::PlatformSettingsRead)?;
    let rows = notifications::list(&state.pool).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| NotificationView {
                id: r.id,
                kind: r.kind,
                title: r.title,
                body: r.body,
                read_at: r.read_at,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

#[utoipa::path(post, path = "/v1/notifications/{id}/read", tag = "ops", responses((status = 204)))]
pub async fn read_notification(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    auth.require(Permission::PlatformSettingsRead)?;
    let id: NotificationId = id.parse().map_err(|_| {
        ApiError(fps_domain::PlatformError::validation(
            "invalid notification id",
        ))
    })?;
    notifications::mark_read(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
