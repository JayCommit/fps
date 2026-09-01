use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{AllocationId, NodeId, ServerId, TemplateId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Pending,
    Installing,
    Running,
    Stopped,
    Failed,
    Deleting,
}

impl ServerStatus {
    pub fn parse(value: &str) -> Self {
        match value {
            "installing" => Self::Installing,
            "running" => Self::Running,
            "stopped" => Self::Stopped,
            "failed" => Self::Failed,
            "deleting" => Self::Deleting,
            _ => Self::Pending,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Installing => "installing",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerSummary {
    pub id: ServerId,
    pub name: String,
    pub template_id: TemplateId,
    pub node_id: Option<NodeId>,
    pub allocation_id: Option<AllocationId>,
    pub status: ServerStatus,
    pub memory_mb: i32,
    pub cpu_shares: i32,
    pub container_name: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
