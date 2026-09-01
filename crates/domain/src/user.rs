use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Role, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserSummary {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub totp_enabled: bool,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
}
