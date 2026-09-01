use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// Stable machine-readable error codes. The frontend maps these; it is not a
/// security boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthenticated,
    Unauthorized,
    SetupRequired,
    SetupAlreadyCompleted,
    InvalidCredentials,
    MfaRequired,
    MfaInvalid,
    Validation,
    Conflict,
    NotFound,
    IdempotencyConflict,
    RateLimited,
    EnrollmentTokenInvalid,
    EnrollmentTokenConsumed,
    NodeUntrusted,
    ProtocolIncompatible,
    InsecureConfiguration,
    Internal,
}

impl ErrorCode {
    pub fn status(self) -> u16 {
        match self {
            Self::Unauthenticated
            | Self::InvalidCredentials
            | Self::MfaRequired
            | Self::MfaInvalid => 401,
            Self::Unauthorized | Self::NodeUntrusted => 403,
            Self::NotFound => 404,
            Self::Conflict | Self::SetupAlreadyCompleted | Self::IdempotencyConflict => 409,
            Self::SetupRequired => 412,
            Self::EnrollmentTokenInvalid | Self::EnrollmentTokenConsumed | Self::Validation => 400,
            Self::RateLimited => 429,
            Self::ProtocolIncompatible | Self::InsecureConfiguration => 422,
            Self::Internal => 500,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Unauthenticated => "Unauthenticated",
            Self::Unauthorized => "Unauthorized",
            Self::SetupRequired => "Setup required",
            Self::SetupAlreadyCompleted => "Setup already completed",
            Self::InvalidCredentials => "Invalid credentials",
            Self::MfaRequired => "Multi-factor authentication required",
            Self::MfaInvalid => "Invalid multi-factor code",
            Self::Validation => "Validation failed",
            Self::Conflict => "Conflict",
            Self::NotFound => "Not found",
            Self::IdempotencyConflict => "Idempotency key reused with a different request",
            Self::RateLimited => "Rate limited",
            Self::EnrollmentTokenInvalid => "Enrollment token is invalid or expired",
            Self::EnrollmentTokenConsumed => "Enrollment token already used",
            Self::NodeUntrusted => "Node identity was rejected",
            Self::ProtocolIncompatible => "Node protocol is incompatible",
            Self::InsecureConfiguration => "Configuration is not safe for this environment",
            Self::Internal => "Internal error",
        }
    }
}

#[derive(Debug, Error, Clone, Serialize)]
#[error("{detail}")]
pub struct PlatformError {
    pub code: ErrorCode,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl PlatformError {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
            field: None,
        }
    }

    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn unauthenticated() -> Self {
        Self::new(ErrorCode::Unauthenticated, "Authentication is required.")
    }

    pub fn unauthorized() -> Self {
        Self::new(
            ErrorCode::Unauthorized,
            "You do not have permission to perform this action.",
        )
    }

    pub fn not_found(resource: &str) -> Self {
        Self::new(ErrorCode::NotFound, format!("{resource} was not found."))
    }

    pub fn validation(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, detail)
    }

    pub fn internal() -> Self {
        Self::new(
            ErrorCode::Internal,
            "An unexpected error occurred. Check server logs with the request id.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthenticated_is_401() {
        assert_eq!(ErrorCode::Unauthenticated.status(), 401);
    }
}
