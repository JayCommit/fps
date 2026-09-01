use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use chrono::Utc;
use fps_auth::{ct_eq_hex, hash_token};
use fps_domain::{Permission, PlatformError, Role, UserId, UserSummary};
use std::net::SocketAddr;

use crate::db::{sessions, users};
use crate::state::AppState;

use super::error::ApiError;

pub struct AuthUser {
    pub user: UserSummary,
    pub role: Role,
    pub session_id: fps_domain::SessionId,
}

impl AuthUser {
    pub fn require(&self, permission: Permission) -> Result<(), ApiError> {
        if Permission::role_has(self.role, permission) {
            Ok(())
        } else {
            Err(ApiError(PlatformError::unauthorized()))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Alpha.1 authenticates the web UI and CLI with Bearer tokens only.
        // Cookie sessions without CSRF are not accepted.
        let Some(token) = bearer_token(parts) else {
            return Err(ApiError(PlatformError::unauthenticated()));
        };
        let hash = hash_token(&token);
        let session = sessions::find_by_token_hash(&state.pool, &hash)
            .await?
            .ok_or_else(|| ApiError(PlatformError::unauthenticated()))?;
        if !ct_eq_hex(&session.token_hash, &hash) {
            return Err(ApiError(PlatformError::unauthenticated()));
        }
        if session.revoked_at.is_some() || session.expires_at < Utc::now() {
            return Err(ApiError(PlatformError::unauthenticated()));
        }
        let user = users::find_by_id(&state.pool, session.user_id)
            .await?
            .ok_or_else(|| ApiError(PlatformError::unauthenticated()))?;
        if matches!(user.status, fps_domain::UserStatus::Disabled) {
            return Err(ApiError(PlatformError::unauthenticated()));
        }
        Ok(Self {
            role: user.role,
            session_id: session.id,
            user: user.summary(),
        })
    }
}

fn bearer_token(parts: &Parts) -> Option<String> {
    let value = parts.headers.get(axum::http::header::AUTHORIZATION)?;
    let value = value.to_str().ok()?;
    value.strip_prefix("Bearer ").map(str::to_string)
}

#[derive(Clone)]
pub struct PeerFingerprint(pub String);

pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.config.trust_forwarded_headers {
            if let Some(fwd) = parts
                .headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
            {
                let first = fwd.split(',').next().unwrap_or(fwd).trim();
                if !first.is_empty() {
                    return Ok(Self(first.to_string()));
                }
            }
        }
        if let Some(ConnectInfo(addr)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            return Ok(Self(addr.ip().to_string()));
        }
        Ok(Self("unknown".into()))
    }
}

pub fn parse_user_id(id: &str) -> Result<UserId, ApiError> {
    id.parse()
        .map_err(|_| ApiError(PlatformError::validation("invalid user id")))
}
