use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use fps_branding::{DISPLAY_NAME, PACKAGE_NAME, VERSION};
use fps_domain::{DATABASE_SCHEMA_VERSION, NODE_PROTOCOL_VERSION};
use serde::Serialize;
use utoipa::ToSchema;

use crate::http::ApiDoc;
use crate::state::AppState;
use utoipa::OpenApi;

use super::super::error::ApiError;

#[derive(Serialize, ToSchema)]
pub struct VersionInfo {
    pub name: String,
    pub package: String,
    pub package: String,
    pub version: String,
    pub api: String,
    pub node_protocol: u16,
    pub database_schema: u32,
    pub channel: String,
}

#[utoipa::path(get, path = "/health", tag = "health", responses((status = 200)))]
pub async fn health() -> StatusCode {
    StatusCode::OK
}

#[utoipa::path(get, path = "/ready", tag = "health", responses((status = 200), (status = 503)))]
pub async fn ready(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(get, path = "/version", tag = "health", responses((status = 200, body = VersionInfo)))]
pub async fn version() -> Json<VersionInfo> {
    Json(VersionInfo {
        name: DISPLAY_NAME.to_string(),
        package: PACKAGE_NAME.to_string(),
        version: VERSION.to_string(),
        api: "v1".into(),
        node_protocol: NODE_PROTOCOL_VERSION,
        database_schema: DATABASE_SCHEMA_VERSION,
        channel: fps_branding::implied_channel().as_str().to_string(),
    })
}

pub async fn metrics() -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        format!(
            "# HELP fps_info Build info\n# TYPE fps_info gauge\nfps_info{{version=\"{VERSION}\"}} 1\n"
        ),
    )
}

pub async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
