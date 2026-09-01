use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use fps_domain::{ErrorCode, PlatformError};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    pub r#type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Problem {
    pub fn from_error(err: &PlatformError) -> Self {
        Self {
            r#type: format!(
                "https://fps.invalid/errors/{}",
                serde_json::to_value(err.code)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| "internal".into())
            ),
            title: err.code.title().to_string(),
            status: err.code.status(),
            detail: err.detail.clone(),
            field: err.field.clone(),
        }
    }
}

pub struct ApiError(pub PlatformError);

impl From<PlatformError> for ApiError {
    fn from(value: PlatformError) -> Self {
        Self(value)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        tracing::error!(error = %value, "database error");
        if cfg!(debug_assertions) {
            Self(PlatformError::new(
                fps_domain::ErrorCode::Internal,
                format!("database error: {value}"),
            ))
        } else {
            Self(PlatformError::internal())
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let problem = Problem::from_error(&self.0);
        let status =
            StatusCode::from_u16(problem.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(problem)).into_response()
    }
}

pub fn map_code(code: ErrorCode, detail: impl Into<String>) -> ApiError {
    ApiError(PlatformError::new(code, detail))
}
