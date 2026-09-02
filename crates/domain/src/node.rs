use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Enrolling,
    Online,
    Degraded,
    Offline,
    Maintenance,
}

impl NodeStatus {
    pub fn from_heartbeat(
        last: Option<DateTime<Utc>>,
        timeout_secs: i64,
        maintenance: bool,
    ) -> Self {
        if maintenance {
            return Self::Maintenance;
        }
        match last {
            None => Self::Enrolling,
            Some(ts) => {
                let age = Utc::now().signed_duration_since(ts);
                if age.num_seconds() <= timeout_secs {
                    Self::Online
                } else if age.num_seconds() <= timeout_secs * 3 {
                    Self::Degraded
                } else {
                    Self::Offline
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DockerState {
    Available,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
#[serde(default)]
pub struct ObservedResources {
    pub cpu_cores: Option<u32>,
    pub memory_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub disk_bytes: Option<u64>,
    pub disk_available_bytes: Option<u64>,
    pub load_one: Option<f32>,
    pub cpu_percent: Option<f32>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeHealth {
    pub id: NodeId,
    pub status: NodeStatus,
    pub docker: DockerState,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub agent_version: Option<String>,
    pub protocol_version: u16,
    pub resources: ObservedResources,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn heartbeat_timeout_marks_offline() {
        let old = Utc::now() - Duration::seconds(400);
        assert_eq!(
            NodeStatus::from_heartbeat(Some(old), 45, false),
            NodeStatus::Offline
        );
        assert_eq!(
            NodeStatus::from_heartbeat(Some(Utc::now()), 45, false),
            NodeStatus::Online
        );
        assert_eq!(
            NodeStatus::from_heartbeat(Some(Utc::now()), 45, true),
            NodeStatus::Maintenance
        );
    }
}
