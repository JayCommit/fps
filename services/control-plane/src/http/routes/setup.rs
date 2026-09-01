use axum::extract::State;
use axum::Json;
use fps_auth::{generate_token, hash_password, hash_token};
use fps_domain::{ErrorCode, Permission, PlatformError, Role, RolePermissions, UserId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{audit, sessions, settings, users};
use crate::http::error::ApiError;
use crate::http::extractors::ClientIp;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct SetupStatus {
    pub completed: bool,
    pub product: String,
    pub version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetupRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[utoipa::path(get, path = "/v1/setup/status", tag = "setup", responses((status = 200, body = SetupStatus)))]
pub async fn setup_status(State(state): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    let completed = settings::setup_completed(&state.pool).await?;
    Ok(Json(SetupStatus {
        completed,
        product: fps_branding::DISPLAY_NAME.to_string(),
        version: fps_branding::VERSION.to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/v1/setup",
    tag = "setup",
    request_body = SetupRequest,
    responses((status = 200, body = crate::http::routes::auth::LoginResponse), (status = 409))
)]
pub async fn complete_setup(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<SetupRequest>,
) -> Result<Json<crate::http::routes::auth::LoginResponse>, ApiError> {
    let email = normalize_email(&body.email)?;
    if body.display_name.trim().len() < 2 {
        return Err(ApiError(
            PlatformError::validation("Display name is required.").field("display_name"),
        ));
    }
    let password_hash = hash_password(&body.password, state.config.argon2)?;
    let user_id = UserId::new();

    let mut tx = state.pool.begin().await?;
    users::lock_setup(&mut tx).await?;
    if settings::setup_completed_tx(&mut tx).await? {
        return Err(ApiError(PlatformError::new(
            ErrorCode::SetupAlreadyCompleted,
            "The owner account already exists. Sign in instead.",
        )));
    }
    users::insert_owner_tx(
        &mut tx,
        user_id,
        &email,
        body.display_name.trim(),
        &password_hash,
    )
    .await?;
    settings::put_json_exec(&mut tx, "setup_completed", &serde_json::json!(true)).await?;
    tx.commit().await?;

    audit::record(
        &state.pool,
        Some(user_id),
        None,
        "setup.completed",
        "platform",
        None,
        Some(&ip),
        None,
        serde_json::json!({ "email": email }),
    )
    .await?;

    let issued = issue_session(&state, user_id, None, Some(&ip)).await?;
    let user = users::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(PlatformError::internal)?;
    Ok(Json(crate::http::routes::auth::LoginResponse {
        user: user.summary(),
        permissions: RolePermissions::for_role(Role::Owner).permissions,
        access_token: issued.access_token,
        refresh_token: issued.refresh_token,
        csrf_token: issued.csrf_token,
        expires_in: state.config.session_ttl_secs,
        mfa_required: false,
        mfa_token: None,
    }))
}

pub fn normalize_email(email: &str) -> Result<String, ApiError> {
    let email = email.trim().to_ascii_lowercase();
    if !email.contains('@') || email.len() < 5 {
        return Err(ApiError(
            PlatformError::validation("Enter a valid email address.").field("email"),
        ));
    }
    Ok(email)
}

pub struct IssuedSession {
    pub access_token: String,
    pub refresh_token: String,
    pub csrf_token: String,
}

pub async fn issue_session(
    state: &AppState,
    user_id: UserId,
    user_agent: Option<&str>,
    ip: Option<&str>,
) -> Result<IssuedSession, ApiError> {
    let access_token = generate_token();
    let refresh_token = generate_token();
    let csrf_token = generate_token();
    let rec = sessions::NewSession {
        id: fps_domain::SessionId::new(),
        user_id,
        token_hash: hash_token(&access_token),
        csrf_token_hash: hash_token(&csrf_token),
        refresh_token_hash: hash_token(&refresh_token),
        user_agent: user_agent.map(str::to_string),
        ip: ip.map(str::to_string),
        expires_at: chrono::Utc::now()
            + chrono::Duration::seconds(state.config.session_ttl_secs as i64),
        refresh_expires_at: chrono::Utc::now()
            + chrono::Duration::seconds(state.config.refresh_ttl_secs as i64),
    };
    sessions::insert(&state.pool, &rec).await?;
    let _ = Permission::PlatformSetup;
    Ok(IssuedSession {
        access_token,
        refresh_token,
        csrf_token,
    })
}
