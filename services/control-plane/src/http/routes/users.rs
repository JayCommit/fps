use axum::extract::{Path, State};
use axum::Json;
use fps_domain::{Permission, Role, UserId, UserStatus, UserSummary};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::db::{audit, users};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchUserRequest {
    pub role: Option<Role>,
    pub status: Option<UserStatus>,
}

#[utoipa::path(get, path = "/v1/users", tag = "identity", responses((status = 200, body = [UserSummary])))]
pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<UserSummary>>, ApiError> {
    auth.require(Permission::IdentityUsersRead)?;
    let records = users::list(&state.pool).await?;
    Ok(Json(records.into_iter().map(|u| u.summary()).collect()))
}

#[utoipa::path(
    patch,
    path = "/v1/users/{id}",
    tag = "identity",
    request_body = PatchUserRequest,
    responses((status = 200, body = UserSummary))
)]
pub async fn patch_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PatchUserRequest>,
) -> Result<Json<UserSummary>, ApiError> {
    auth.require(Permission::IdentityUsersWrite)?;
    let id: UserId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid user id")))?;
    let existing = users::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("user")))?;
    if let Some(status) = body.status {
        if matches!(status, UserStatus::Disabled)
            && existing.role == Role::Owner
            && users::count_owners(&state.pool).await? <= 1
        {
            return Err(ApiError(fps_domain::PlatformError::validation(
                "Cannot disable the last owner.",
            )));
        }
        users::set_status(&state.pool, id, status).await?;
    }
    if let Some(role) = body.role {
        if existing.role == Role::Owner
            && role != Role::Owner
            && users::count_owners(&state.pool).await? <= 1
        {
            return Err(ApiError(fps_domain::PlatformError::validation(
                "Cannot demote the last owner.",
            )));
        }
        users::set_role(&state.pool, id, role).await?;
    }
    let updated = users::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("user")))?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        None,
        "identity.user.updated",
        "user",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "role": body.role, "status": body.status }),
    )
    .await?;
    Ok(Json(updated.summary()))
}
