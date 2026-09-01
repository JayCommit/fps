use axum::extract::State;
use axum::Json;
use chrono::Utc;
use fps_auth::{
    ct_eq_hex, decrypt_totp_secret, encrypt_totp_secret, generate_recovery_codes,
    generate_totp_secret, hash_recovery_code, hash_token, totp_otpauth_url, verify_password,
    verify_recovery_code, verify_totp,
};
use fps_domain::{ErrorCode, PlatformError, RolePermissions};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{audit, sessions, users};
use crate::http::error::ApiError;
use crate::http::extractors::{AuthUser, ClientIp};
use crate::http::routes::setup::{issue_session, normalize_email};
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: Option<String>,
    pub recovery_code: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub user: fps_domain::UserSummary,
    pub permissions: Vec<fps_domain::Permission>,
    pub access_token: String,
    pub refresh_token: String,
    pub csrf_token: String,
    pub expires_in: u64,
    pub mfa_required: bool,
    pub mfa_token: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MeResponse {
    pub user: fps_domain::UserSummary,
    pub permissions: Vec<fps_domain::Permission>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TotpStartResponse {
    pub otpauth_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TotpConfirmRequest {
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TotpConfirmResponse {
    pub recovery_codes: Vec<String>,
}

#[utoipa::path(post, path = "/v1/auth/login", tag = "auth", request_body = LoginRequest, responses((status = 200, body = LoginResponse), (status = 401)))]
pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    if state.login_attempts.get(&ip).map(|v| v.len()).unwrap_or(0) > 10 {
        return Err(ApiError(PlatformError::new(
            ErrorCode::RateLimited,
            "Too many sign-in attempts. Wait a minute and try again.",
        )));
    }
    let email = normalize_email(&body.email)?;
    let user = users::find_by_email(&state.pool, &email).await?;
    let Some(user) = user else {
        let _ = state.record_login_failure(&ip);
        return Err(ApiError(PlatformError::new(
            ErrorCode::InvalidCredentials,
            "Email or password is incorrect.",
        )));
    };
    let stored = users::password_hash(&state.pool, user.id)
        .await?
        .ok_or_else(|| {
            ApiError(PlatformError::new(
                ErrorCode::InvalidCredentials,
                "Email or password is incorrect.",
            ))
        })?;
    if !verify_password(&body.password, &stored)? {
        let limited = state.record_login_failure(&ip);
        if limited {
            return Err(ApiError(PlatformError::new(
                ErrorCode::RateLimited,
                "Too many sign-in attempts. Wait a minute and try again.",
            )));
        }
        return Err(ApiError(PlatformError::new(
            ErrorCode::InvalidCredentials,
            "Email or password is incorrect.",
        )));
    }
    if user.totp_enabled {
        let mut hashes = user.recovery_hashes();
        let recovery_ok = body
            .recovery_code
            .as_deref()
            .and_then(|code| verify_recovery_code(code, &hashes));
        if let Some(idx) = recovery_ok {
            hashes.remove(idx);
            users::replace_recovery_hashes(&state.pool, user.id, &hashes).await?;
        } else {
            let Some(code) = body.totp_code.as_deref() else {
                return Err(ApiError(PlatformError::new(
                    ErrorCode::MfaRequired,
                    "Enter the six-digit code from your authenticator app, or a recovery code.",
                )));
            };
            let encrypted = user.totp_secret_encrypted.as_deref().ok_or_else(|| {
                ApiError(PlatformError::new(
                    ErrorCode::Internal,
                    "MFA is enabled but no secret is stored.",
                ))
            })?;
            let secret = decrypt_totp_secret(&state.master_key, encrypted)
                .map_err(|_| ApiError(PlatformError::internal()))?;
            if !verify_totp(&secret, &user.email, code)? {
                return Err(ApiError(PlatformError::new(
                    ErrorCode::MfaInvalid,
                    "That authentication code is not valid.",
                )));
            }
        }
    }
    state.clear_login_failures(&ip);
    let issued = issue_session(&state, user.id, None, Some(&ip)).await?;
    audit::record(
        &state.pool,
        Some(user.id),
        None,
        "auth.login",
        "session",
        None,
        Some(&ip),
        None,
        serde_json::json!({}),
    )
    .await?;
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

#[utoipa::path(post, path = "/v1/auth/logout", tag = "auth", responses((status = 204)))]
pub async fn logout(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<StatusCodeWrapper, ApiError> {
    sessions::revoke(&state.pool, auth.session_id).await?;
    Ok(StatusCodeWrapper(axum::http::StatusCode::NO_CONTENT))
}

pub struct StatusCodeWrapper(pub axum::http::StatusCode);

impl axum::response::IntoResponse for StatusCodeWrapper {
    fn into_response(self) -> axum::response::Response {
        self.0.into_response()
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[utoipa::path(post, path = "/v1/auth/refresh", tag = "auth", responses((status = 200, body = LoginResponse)))]
pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let hash = hash_token(&body.refresh_token);
    let session = sessions::find_by_refresh_hash(&state.pool, &hash)
        .await?
        .ok_or_else(|| ApiError(PlatformError::unauthenticated()))?;
    if session.revoked_at.is_some() {
        return Err(ApiError(PlatformError::unauthenticated()));
    }
    let Some(refresh_expires_at) = session.refresh_expires_at else {
        return Err(ApiError(PlatformError::unauthenticated()));
    };
    if refresh_expires_at < Utc::now() {
        return Err(ApiError(PlatformError::unauthenticated()));
    }
    if let Some(stored) = session.refresh_token_hash.as_deref() {
        if !ct_eq_hex(stored, &hash) {
            return Err(ApiError(PlatformError::unauthenticated()));
        }
    }
    let access_token = fps_auth::generate_token();
    let refresh_token = fps_auth::generate_token();
    let csrf_token = fps_auth::generate_token();
    sessions::rotate(
        &state.pool,
        session.id,
        &hash_token(&access_token),
        &hash_token(&refresh_token),
        &hash_token(&csrf_token),
        Utc::now() + chrono::Duration::seconds(state.config.session_ttl_secs as i64),
        Utc::now() + chrono::Duration::seconds(state.config.refresh_ttl_secs as i64),
    )
    .await?;
    let user = users::find_by_id(&state.pool, session.user_id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::unauthenticated()))?;
    Ok(Json(LoginResponse {
        permissions: RolePermissions::for_role(user.role).permissions,
        user: user.summary(),
        access_token,
        refresh_token,
        csrf_token,
        expires_in: state.config.session_ttl_secs,
        mfa_required: false,
        mfa_token: None,
    }))
}

#[utoipa::path(get, path = "/v1/auth/me", tag = "auth", responses((status = 200, body = MeResponse)))]
pub async fn me(auth: AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        permissions: RolePermissions::for_role(auth.role).permissions,
        user: auth.user,
    })
}

#[utoipa::path(post, path = "/v1/auth/totp/start", tag = "auth", responses((status = 200, body = TotpStartResponse)))]
pub async fn enable_totp(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<TotpStartResponse>, ApiError> {
    let secret = generate_totp_secret();
    let encrypted = encrypt_totp_secret(&state.master_key, &secret)
        .map_err(|_| ApiError(PlatformError::internal()))?;
    users::set_totp_pending(&state.pool, auth.user.id, &encrypted).await?;
    let url = totp_otpauth_url(&secret, &auth.user.email)?;
    Ok(Json(TotpStartResponse { otpauth_url: url }))
}

#[utoipa::path(post, path = "/v1/auth/totp/confirm", tag = "auth", request_body = TotpConfirmRequest, responses((status = 200, body = TotpConfirmResponse)))]
pub async fn confirm_totp(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<TotpConfirmRequest>,
) -> Result<Json<TotpConfirmResponse>, ApiError> {
    let user = users::find_by_id(&state.pool, auth.user.id)
        .await?
        .ok_or_else(|| ApiError(PlatformError::unauthenticated()))?;
    let encrypted = user.totp_pending_encrypted.ok_or_else(|| {
        ApiError(PlatformError::validation(
            "Start TOTP enrollment before confirming.",
        ))
    })?;
    let secret = decrypt_totp_secret(&state.master_key, &encrypted)
        .map_err(|_| ApiError(PlatformError::internal()))?;
    if !verify_totp(&secret, &user.email, &body.code)? {
        return Err(ApiError(PlatformError::new(
            ErrorCode::MfaInvalid,
            "That authentication code is not valid.",
        )));
    }
    let codes = generate_recovery_codes();
    let hashes: Vec<String> = codes.iter().map(|c| hash_recovery_code(c)).collect();
    users::set_totp(&state.pool, user.id, &encrypted, &hashes).await?;
    audit::record(
        &state.pool,
        Some(user.id),
        None,
        "auth.totp.enabled",
        "user",
        Some(&user.id.to_string()),
        None,
        None,
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(TotpConfirmResponse {
        recovery_codes: codes,
    }))
}
