//! Wire types for node enrollment, heartbeat, jobs, and capability reporting.
//!
//! Versioning: increment [`PROTOCOL_VERSION`] when fields become required or
//! semantics change. Agents refuse instructions from incompatible control planes.
//! Optional heartbeat fields use `#[serde(default)]` so protocol v1 stays compatible.

use chrono::{DateTime, Utc};
use fps_domain::{
    DockerState, JobId, JobKind, NodeId, ObservedResources, ServerId, NODE_PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const PROTOCOL_VERSION: u16 = NODE_PROTOCOL_VERSION;
pub const MIN_SUPPORTED_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnrollRequest {
    pub enrollment_token: String,
    pub hostname: String,
    pub name: Option<String>,
    pub agent_version: String,
    pub protocol_version: u16,
    pub architecture: String,
    pub operating_system: String,
    pub labels: Vec<String>,
    pub docker: DockerCapability,
    pub resources: ObservedResources,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DockerCapability {
    pub state: DockerState,
    pub engine_version: Option<String>,
    pub api_version: Option<String>,
    pub cgroup_version: Option<String>,
    pub error: Option<String>,
}

impl Default for DockerCapability {
    fn default() -> Self {
        Self {
            state: DockerState::Unavailable,
            engine_version: None,
            api_version: None,
            cgroup_version: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnrollResponse {
    pub node_id: NodeId,
    pub node_token: String,
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub ca_pem: String,
    pub heartbeat_interval_seconds: u64,
    pub protocol_version: u16,
    pub control_plane_version: String,
    pub node_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobInstruction {
    pub id: JobId,
    pub kind: JobKind,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobResult {
    pub id: JobId,
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub container_name: Option<String>,
    #[serde(default)]
    pub log_excerpt: Option<String>,
    #[serde(default)]
    pub backup_path: Option<String>,
    #[serde(default)]
    pub backup_bytes: Option<u64>,
    #[serde(default)]
    pub files: Option<serde_json::Value>,
    #[serde(default)]
    pub file_content: Option<String>,
    #[serde(default)]
    pub tracked_paths: Option<Vec<String>>,
    /// Optional machine code (`port_conflict`) so the control plane can retry.
    #[serde(default)]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LogChunk {
    pub server_id: ServerId,
    pub stream: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HeartbeatRequest {
    pub protocol_version: u16,
    pub agent_version: String,
    pub docker: DockerCapability,
    pub resources: ObservedResources,
    pub started_at: DateTime<Utc>,
    pub workload_count: u32,
    pub note: Option<String>,
    #[serde(default)]
    pub job_results: Vec<JobResult>,
    #[serde(default)]
    pub log_chunks: Vec<LogChunk>,
    #[serde(default)]
    pub container_samples: Vec<ContainerSample>,
    #[serde(default)]
    pub control_ack: Option<NodeControlAck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContainerSample {
    pub server_id: ServerId,
    pub running: bool,
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    #[serde(default)]
    pub cpu_percent: Option<f32>,
    #[serde(default)]
    pub restart_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct NodeControlAck {
    #[serde(default)]
    pub uninstall: Option<String>,
    #[serde(default)]
    pub docker_prune: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct NodeControlSettings {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(default)]
    pub heartbeat_interval_seconds: Option<u64>,
    #[serde(default)]
    pub maintenance: Option<bool>,
    #[serde(default)]
    pub uninstall: bool,
    #[serde(default)]
    pub docker_prune: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HeartbeatResponse {
    pub accepted: bool,
    pub protocol_version: u16,
    pub server_time: DateTime<Utc>,
    pub rotate_token: Option<String>,
    pub desired_drain: bool,
    #[serde(default)]
    pub jobs: Vec<JobInstruction>,
    #[serde(default)]
    pub settings: NodeControlSettings,
}

pub fn protocol_compatible(agent: u16, control_plane: u16) -> bool {
    agent >= MIN_SUPPORTED_PROTOCOL_VERSION && agent <= control_plane
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_protocol_is_compatible_with_itself() {
        assert!(protocol_compatible(PROTOCOL_VERSION, PROTOCOL_VERSION));
        assert!(!protocol_compatible(0, PROTOCOL_VERSION));
        assert!(!protocol_compatible(PROTOCOL_VERSION + 1, PROTOCOL_VERSION));
    }

    #[test]
    fn heartbeat_defaults_keep_v1_compatible() {
        let json = r#"{
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": {"state": "unavailable"},
            "resources": {},
            "started_at": "2026-01-01T00:00:00Z",
            "workload_count": 0
        }"#;
        let req: HeartbeatRequest = serde_json::from_str(json).unwrap();
        assert!(req.job_results.is_empty());
        assert!(req.control_ack.is_none());
        let resp: HeartbeatResponse = serde_json::from_str(
            r#"{"accepted":true,"protocol_version":1,"server_time":"2026-01-01T00:00:00Z","rotate_token":null,"desired_drain":false}"#,
        )
        .unwrap();
        assert!(resp.jobs.is_empty());
        assert!(!resp.settings.uninstall);
        assert!(!resp.settings.docker_prune);
    }

    #[test]
    fn control_settings_round_trip() {
        let settings = NodeControlSettings {
            name: Some("edge-1".into()),
            labels: Some(vec!["site:lab".into()]),
            heartbeat_interval_seconds: Some(30),
            maintenance: Some(true),
            uninstall: true,
            docker_prune: false,
        };
        let json = serde_json::to_value(&settings).unwrap();
        let back: NodeControlSettings = serde_json::from_value(json).unwrap();
        assert_eq!(back.name.as_deref(), Some("edge-1"));
        assert_eq!(back.heartbeat_interval_seconds, Some(30));
        assert!(back.uninstall);
        assert!(back.maintenance.unwrap());
    }
}
