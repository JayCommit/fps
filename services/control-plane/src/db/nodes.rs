use chrono::{DateTime, Utc};
use fps_domain::{DockerState, NodeHealth, NodeId, NodeStatus, ObservedResources, UserId};
use fps_protocol::DockerCapability;
use sqlx::{MySql, MySqlPool, Transaction};

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct NodeRecord {
    pub id: NodeId,
    pub name: String,
    pub hostname: String,
    pub status: String,
    pub agent_version: Option<String>,
    pub protocol_version: i32,
    pub architecture: Option<String>,
    pub operating_system: Option<String>,
    pub labels: Vec<String>,
    pub docker_state: String,
    pub docker_engine_version: Option<String>,
    pub docker_error: Option<String>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub maintenance: bool,
    pub enrolled_at: DateTime<Utc>,
    pub cpu_cores: Option<i32>,
    pub memory_bytes: Option<i64>,
    pub disk_bytes: Option<i64>,
    pub disk_available_bytes: Option<i64>,
    pub load_one: Option<f32>,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<i64>,
    pub uptime_seconds: Option<i64>,
    pub heartbeat_interval_seconds: u64,
    pub docker_prune_requested: bool,
    pub uninstall_requested_at: Option<DateTime<Utc>>,
    pub uninstalled_at: Option<DateTime<Utc>>,
    pub health_message: Option<String>,
    pub workload_count: i32,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl NodeRecord {
    pub fn health(&self, timeout_secs: i64) -> NodeHealth {
        let maintenance = self.maintenance;
        let status = NodeStatus::from_heartbeat(self.last_heartbeat_at, timeout_secs, maintenance);
        NodeHealth {
            id: self.id,
            status,
            docker: parse_docker(&self.docker_state),
            last_heartbeat_at: self.last_heartbeat_at,
            agent_version: self.agent_version.clone(),
            protocol_version: self.protocol_version as u16,
            resources: ObservedResources {
                cpu_cores: self.cpu_cores.map(|v| v as u32),
                memory_bytes: self.memory_bytes.map(|v| v as u64),
                memory_used_bytes: self.memory_used_bytes.map(|v| v as u64),
                disk_bytes: self.disk_bytes.map(|v| v as u64),
                disk_available_bytes: self.disk_available_bytes.map(|v| v as u64),
                load_one: self.load_one,
                cpu_percent: self.cpu_percent.map(|v| v as f32),
                uptime_seconds: self.uptime_seconds.map(|v| v as u64),
            },
            message: self
                .health_message
                .clone()
                .unwrap_or_else(|| status_message(status, parse_docker(&self.docker_state))),
        }
    }
}

fn parse_docker(value: &str) -> DockerState {
    match value {
        "available" => DockerState::Available,
        "error" => DockerState::Error,
        _ => DockerState::Unavailable,
    }
}

fn status_message(status: NodeStatus, docker: DockerState) -> String {
    match (status, docker) {
        (NodeStatus::Online, DockerState::Available) => {
            "Heartbeat received. Docker engine is reachable.".into()
        }
        (NodeStatus::Online, _) => {
            "Heartbeat received. Docker is not available on this node.".into()
        }
        (NodeStatus::Degraded, _) => "Heartbeat is late. The node may be under pressure.".into(),
        (NodeStatus::Offline, _) => "No recent heartbeat.".into(),
        (NodeStatus::Maintenance, _) => "Node is in maintenance mode.".into(),
        (NodeStatus::Enrolling, _) => "Waiting for the first heartbeat.".into(),
    }
}

pub struct NewNode<'a> {
    pub id: NodeId,
    pub name: &'a str,
    pub hostname: &'a str,
    pub agent_version: &'a str,
    pub protocol_version: u16,
    pub architecture: &'a str,
    pub os: &'a str,
    pub labels: &'a [String],
    pub docker: &'a DockerCapability,
    pub fingerprint: &'a str,
    pub token_hash: &'a str,
    pub resources: &'a ObservedResources,
}

pub async fn insert_node_tx(
    tx: &mut Transaction<'_, MySql>,
    rec: &NewNode<'_>,
) -> Result<(), sqlx::Error> {
    let now = now_utc();
    let labels_json = serde_json::to_string(rec.labels).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "INSERT INTO nodes (
            id, name, hostname, status, agent_version, protocol_version, architecture, operating_system,
            labels_json, docker_state, docker_engine_version, docker_error, last_heartbeat_at, maintenance,
            enrolled_at, certificate_fingerprint, token_hash, cpu_cores, memory_bytes, disk_bytes,
            disk_available_bytes, load_one, cpu_percent, memory_used_bytes, uptime_seconds,
            heartbeat_interval_seconds, health_message, workload_count, created_at, updated_at
        ) VALUES (?, ?, ?, 'enrolling', ?, ?, ?, ?, ?, ?, ?, ?, NULL, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 15, ?, 0, ?, ?)",
    )
    .bind(rec.id.to_string())
    .bind(rec.name)
    .bind(rec.hostname)
    .bind(rec.agent_version)
    .bind(rec.protocol_version as i32)
    .bind(rec.architecture)
    .bind(rec.os)
    .bind(labels_json)
    .bind(docker_state_str(rec.docker.state))
    .bind(&rec.docker.engine_version)
    .bind(&rec.docker.error)
    .bind(now)
    .bind(rec.fingerprint)
    .bind(rec.token_hash)
    .bind(rec.resources.cpu_cores.map(|v| v as i32))
    .bind(rec.resources.memory_bytes.map(|v| v as i64))
    .bind(rec.resources.disk_bytes.map(|v| v as i64))
    .bind(rec.resources.disk_available_bytes.map(|v| v as i64))
    .bind(rec.resources.load_one)
    .bind(rec.resources.cpu_percent.map(f64::from))
    .bind(rec.resources.memory_used_bytes.map(|v| v as i64))
    .bind(rec.resources.uptime_seconds.map(|v| v as i64))
    .bind("Waiting for the first heartbeat.")
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn docker_state_str(state: DockerState) -> &'static str {
    match state {
        DockerState::Available => "available",
        DockerState::Unavailable => "unavailable",
        DockerState::Error => "error",
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn heartbeat(
    pool: &MySqlPool,
    id: NodeId,
    agent_version: &str,
    protocol_version: u16,
    docker: &DockerCapability,
    resources: &ObservedResources,
    workload_count: u32,
    note: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = now_utc();
    sqlx::query(
        "UPDATE nodes SET
            status = 'online',
            agent_version = ?,
            protocol_version = ?,
            docker_state = ?,
            docker_engine_version = ?,
            docker_error = ?,
            last_heartbeat_at = ?,
            cpu_cores = ?,
            memory_bytes = ?,
            disk_bytes = ?,
            disk_available_bytes = ?,
            load_one = ?,
            cpu_percent = ?,
            memory_used_bytes = ?,
            uptime_seconds = ?,
            health_message = ?,
            workload_count = ?,
            updated_at = ?
         WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(agent_version)
    .bind(protocol_version as i32)
    .bind(docker_state_str(docker.state))
    .bind(&docker.engine_version)
    .bind(&docker.error)
    .bind(now)
    .bind(resources.cpu_cores.map(|v| v as i32))
    .bind(resources.memory_bytes.map(|v| v as i64))
    .bind(resources.disk_bytes.map(|v| v as i64))
    .bind(resources.disk_available_bytes.map(|v| v as i64))
    .bind(resources.load_one)
    .bind(resources.cpu_percent.map(f64::from))
    .bind(resources.memory_used_bytes.map(|v| v as i64))
    .bind(resources.uptime_seconds.map(|v| v as i64))
    .bind(note.unwrap_or("Heartbeat received."))
    .bind(workload_count as i32)
    .bind(now)
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

const NODE_COLS: &str = "SELECT id, name, hostname, status, agent_version, protocol_version, architecture, operating_system,
                CAST(labels_json AS CHAR) AS labels_json,
                docker_state, docker_engine_version, docker_error, last_heartbeat_at, maintenance, enrolled_at,
                cpu_cores, memory_bytes, disk_bytes, disk_available_bytes, load_one, cpu_percent, memory_used_bytes,
                uptime_seconds, heartbeat_interval_seconds, docker_prune_requested, uninstall_requested_at,
                uninstalled_at, health_message, workload_count, revoked_at
         FROM nodes";

pub async fn list(pool: &MySqlPool) -> Result<Vec<NodeRecord>, sqlx::Error> {
    let rows =
        sqlx::query_as::<_, NodeRow>(&format!("{NODE_COLS} ORDER BY created_at DESC LIMIT 500"))
            .fetch_all(pool)
            .await?;
    rows.into_iter().map(NodeRecord::try_from).collect()
}

pub async fn get(pool: &MySqlPool, id: NodeId) -> Result<Option<NodeRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, NodeRow>(&format!("{NODE_COLS} WHERE id = ?"))
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    row.map(NodeRecord::try_from).transpose()
}

pub async fn find_token_hash(pool: &MySqlPool, id: NodeId) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT token_hash FROM nodes WHERE id = ? AND revoked_at IS NULL")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

pub async fn revoke(pool: &MySqlPool, id: NodeId) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE nodes SET revoked_at = ?, status = 'offline', updated_at = ? WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(now_utc())
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub struct NodeSettingsUpdate<'a> {
    pub name: &'a str,
    pub labels: &'a [String],
    pub maintenance: bool,
    pub heartbeat_interval_seconds: u64,
}

pub async fn update_settings(
    pool: &MySqlPool,
    id: NodeId,
    update: &NodeSettingsUpdate<'_>,
) -> Result<u64, sqlx::Error> {
    let labels_json = serde_json::to_string(update.labels).unwrap_or_else(|_| "[]".into());
    let res = sqlx::query(
        "UPDATE nodes SET name = ?, labels_json = ?, maintenance = ?, heartbeat_interval_seconds = ?, updated_at = ?
         WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(update.name)
    .bind(labels_json)
    .bind(i8::from(update.maintenance))
    .bind(update.heartbeat_interval_seconds as i32)
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn request_uninstall(pool: &MySqlPool, id: NodeId) -> Result<u64, sqlx::Error> {
    let now = now_utc();
    let res = sqlx::query(
        "UPDATE nodes SET uninstall_requested_at = ?, maintenance = 1, updated_at = ?
         WHERE id = ? AND revoked_at IS NULL AND uninstalled_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn mark_uninstalled(pool: &MySqlPool, id: NodeId) -> Result<(), sqlx::Error> {
    let now = now_utc();
    sqlx::query(
        "UPDATE nodes SET uninstalled_at = ?, maintenance = 1, docker_prune_requested = 0, updated_at = ?
         WHERE id = ?",
    )
    .bind(now)
    .bind(now)
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn request_docker_prune(pool: &MySqlPool, id: NodeId) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE nodes SET docker_prune_requested = 1, updated_at = ? WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn clear_docker_prune(pool: &MySqlPool, id: NodeId) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE nodes SET docker_prune_requested = 0, updated_at = ? WHERE id = ?")
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn pick_schedulable(pool: &MySqlPool) -> Result<Option<NodeRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, NodeRow>(&format!(
        "{NODE_COLS}
         WHERE revoked_at IS NULL AND maintenance = 0 AND docker_state = 'available'
           AND last_heartbeat_at IS NOT NULL
           AND last_heartbeat_at > DATE_SUB(UTC_TIMESTAMP(3), INTERVAL 90 SECOND)
         ORDER BY workload_count ASC, disk_available_bytes DESC, last_heartbeat_at DESC LIMIT 1"
    ))
    .fetch_optional(pool)
    .await?;
    row.map(NodeRecord::try_from).transpose()
}

pub async fn find_id_by_fingerprint(
    pool: &MySqlPool,
    fingerprint: &str,
) -> Result<Option<NodeId>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM nodes WHERE certificate_fingerprint = ? AND revoked_at IS NULL",
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((id,)) => Ok(Some(parse_id(&id, "nodes.id")?)),
        None => Ok(None),
    }
}

pub async fn insert_enrollment_token(
    pool: &MySqlPool,
    id: &str,
    token_hash: &str,
    label: Option<&str>,
    created_by: UserId,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO node_enrollment_tokens (id, token_hash, label, created_by, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(token_hash)
    .bind(label)
    .bind(created_by.to_string())
    .bind(expires_at.naive_utc())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub struct EnrollmentTokenRow {
    pub id: String,
    pub consumed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

pub async fn lock_enrollment_token(
    tx: &mut Transaction<'_, MySql>,
    token_hash: &str,
) -> Result<Option<EnrollmentTokenRow>, sqlx::Error> {
    let row: Option<(String, Option<chrono::NaiveDateTime>, chrono::NaiveDateTime)> =
        sqlx::query_as(
            "SELECT id, consumed_at, expires_at FROM node_enrollment_tokens WHERE token_hash = ? FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.map(|(id, consumed_at, expires_at)| EnrollmentTokenRow {
        id,
        consumed_at: consumed_at.map(from_naive),
        expires_at: from_naive(expires_at),
    }))
}

pub async fn consume_enrollment_token_tx(
    tx: &mut Transaction<'_, MySql>,
    id: &str,
    node_id: NodeId,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE node_enrollment_tokens SET consumed_at = ?, consumed_by_node = ?
         WHERE id = ? AND consumed_at IS NULL",
    )
    .bind(now_utc())
    .bind(node_id.to_string())
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected())
}

#[derive(sqlx::FromRow)]
struct NodeRow {
    id: String,
    name: String,
    hostname: String,
    status: String,
    agent_version: Option<String>,
    protocol_version: i32,
    architecture: Option<String>,
    operating_system: Option<String>,
    labels_json: Option<String>,
    docker_state: String,
    docker_engine_version: Option<String>,
    docker_error: Option<String>,
    last_heartbeat_at: Option<chrono::NaiveDateTime>,
    maintenance: i8,
    enrolled_at: chrono::NaiveDateTime,
    cpu_cores: Option<i32>,
    memory_bytes: Option<i64>,
    disk_bytes: Option<i64>,
    disk_available_bytes: Option<i64>,
    load_one: Option<f32>,
    cpu_percent: Option<f64>,
    memory_used_bytes: Option<i64>,
    uptime_seconds: Option<i64>,
    heartbeat_interval_seconds: i32,
    docker_prune_requested: i8,
    uninstall_requested_at: Option<chrono::NaiveDateTime>,
    uninstalled_at: Option<chrono::NaiveDateTime>,
    health_message: Option<String>,
    workload_count: i32,
    revoked_at: Option<chrono::NaiveDateTime>,
}

fn parse_labels(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
}

impl TryFrom<NodeRow> for NodeRecord {
    type Error = sqlx::Error;

    fn try_from(row: NodeRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "nodes.id")?,
            name: row.name,
            hostname: row.hostname,
            status: row.status,
            agent_version: row.agent_version,
            protocol_version: row.protocol_version,
            architecture: row.architecture,
            operating_system: row.operating_system,
            labels: parse_labels(row.labels_json.as_deref()),
            docker_state: row.docker_state,
            docker_engine_version: row.docker_engine_version,
            docker_error: row.docker_error,
            last_heartbeat_at: row.last_heartbeat_at.map(from_naive),
            maintenance: row.maintenance != 0,
            enrolled_at: from_naive(row.enrolled_at),
            cpu_cores: row.cpu_cores,
            memory_bytes: row.memory_bytes,
            disk_bytes: row.disk_bytes,
            disk_available_bytes: row.disk_available_bytes,
            load_one: row.load_one,
            cpu_percent: row.cpu_percent,
            memory_used_bytes: row.memory_used_bytes,
            uptime_seconds: row.uptime_seconds,
            heartbeat_interval_seconds: row.heartbeat_interval_seconds.max(5) as u64,
            docker_prune_requested: row.docker_prune_requested != 0,
            uninstall_requested_at: row.uninstall_requested_at.map(from_naive),
            uninstalled_at: row.uninstalled_at.map(from_naive),
            health_message: row.health_message,
            workload_count: row.workload_count,
            revoked_at: row.revoked_at.map(from_naive),
        })
    }
}
