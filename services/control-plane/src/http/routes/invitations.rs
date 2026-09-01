use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use fps_auth::{generate_token, hash_password, hash_token};
use fps_domain::{InvitationId, Permission, Role, RolePermissions, UserId, UserStatus};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{audit, invitations, users};
use crate::http::error::ApiError;
use crate::http::extractors::{AuthUser, ClientIp};
use crate::http::routes::auth::LoginResponse;
use crate::http::routes::setup::{issue_session, normalize_email};
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationView {
    pub id: InvitationId,
    pub email: String,
    pub role: Role,
    pub expires_at: chrono::DateTime<Utc>,
    pub accepted_at: Option<chrono::DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AcceptInvitationRequest {
    pub token: String,
    pub password: String,
    pub display_name: String,
}

#[utoipa::path(get, path = "/v1/invitations", tag = "identity", responses((status = 200, body = [InvitationView])))]
pub async fn list_invitations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<InvitationView>>, ApiError> {
    auth.require(Permission::IdentityUsersRead)?;
    let rows = invitations::list(&state.pool).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| InvitationView {
                id: r.id,
                email: r.email,
                role: r.role,
                expires_at: r.expires_at,
                accepted_at: r.accepted_at,
                token: None,
            })
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/invitations",
    tag = "identity",
    request_body = CreateInvitationRequest,
    responses((status = 200, body = InvitationView))
)]
pub async fn create_invitation(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateInvitationRequest>,
) -> Result<Json<InvitationView>, ApiError> {
    auth.require(Permission::IdentityUsersWrite)?;
    if matches!(body.role, Role::Owner) {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "Owner cannot be invited. There is exactly one owner from setup.",
        )));
    }
    let email = normalize_email(&body.email)?;
    if users::find_by_email(&state.pool, &email).await?.is_some() {
        return Err(ApiError(fps_domain::PlatformError::new(
            fps_domain::ErrorCode::Conflict,
            "A user with that email already exists.",
        )));
    }
    let token = generate_token();
    let id = InvitationId::new();
    let expires_at = Utc::now() + Duration::days(7);
    invitations::insert(
        &state.pool,
        id,
        &email,
        body.role,
        &hash_token(&token),
        auth.user.id,
        expires_at,
    )
    .await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        None,
        "identity.invitation.created",
        "invitation",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "email": email, "role": body.role }),
    )
    .await?;
    Ok(Json(InvitationView {
        id,
        email,
        role: body.role,
        expires_at,
        accepted_at: None,
        token: Some(token),
    }))
}

#[utoipa::path(
    post,
    path = "/v1/invitations/accept",
    tag = "identity",
    request_body = AcceptInvitationRequest,
    responses((status = 200, body = LoginResponse))
)]
pub async fn accept_invitation(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<AcceptInvitationRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let rec = invitations::find_by_token_hash(&state.pool, &hash_token(&body.token))
        .await?
        .ok_or_else(|| {
            ApiError(fps_domain::PlatformError::validation(
                "Invitation token is invalid.",
            ))
        })?;
    if rec.accepted_at.is_some() || rec.expires_at < Utc::now() {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "Invitation is expired or already accepted.",
        )));
    }
    if users::find_by_email(&state.pool, &rec.email)
        .await?
        .is_some()
    {
        return Err(ApiError(fps_domain::PlatformError::new(
            fps_domain::ErrorCode::Conflict,
            "A user with that email already exists.",
        )));
    }
    let display = body.display_name.trim();
    if display.is_empty() {
        return Err(ApiError(
            fps_domain::PlatformError::validation("Display name is required.")
                .field("display_name"),
        ));
    }
    let password_hash = hash_password(&body.password, state.config.argon2)?;
    let user_id = UserId::new();
    users::insert_user(
        &state.pool,
        user_id,
        &rec.email,
        display,
        rec.role,
        &password_hash,
    )
    .await?;
    if invitations::mark_accepted(&state.pool, rec.id).await? != 1 {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "Invitation is expired or already accepted.",
        )));
    }
    let issued = issue_session(&state, user_id, None, Some(&ip)).await?;
    let user = users::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::internal()))?;
    audit::record(
        &state.pool,
        Some(user_id),
        None,
        "identity.invitation.accepted",
        "invitation",
        Some(&rec.id.to_string()),
        Some(&ip),
        None,
        serde_json::json!({ "email": rec.email }),
    )
    .await?;
    let _ = UserStatus::Active;
    Ok(Json(LoginResponse {
        permissions: RolePermissions::for_role(user.role).permissions,
        user: user.summary(),
        access_token: issued.access_token,
        refresh_token: issued.refresh_token,
        csrf_token: issued.csrf_token,
        expires_in: state.config.session_ttl_secs,
        mfa_required: false,
        mfa_token: None,
    }))
}
