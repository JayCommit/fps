use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{AddonInstallId, JobId, ServerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AddonInstallStatus {
    Queued,
    Installed,
    Uninstalling,
    Failed,
}

impl AddonInstallStatus {
    pub fn parse(value: &str) -> Self {
        match value {
            "installed" => Self::Installed,
            "uninstalling" => Self::Uninstalling,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Installed => "installed",
            Self::Uninstalling => "uninstalling",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerAddonSummary {
    pub id: AddonInstallId,
    pub server_id: ServerId,
    pub addon_slug: String,
    pub addon_name: String,
    pub version_label: String,
    pub status: AddonInstallStatus,
    pub tracked_paths: Vec<String>,
    pub job_id: Option<JobId>,
    pub error: Option<String>,
    pub installed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        for status in [
            AddonInstallStatus::Queued,
            AddonInstallStatus::Installed,
            AddonInstallStatus::Uninstalling,
            AddonInstallStatus::Failed,
        ] {
            assert_eq!(AddonInstallStatus::parse(status.as_str()), status);
        }
    }
}
