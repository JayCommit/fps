use chrono::{DateTime, Utc};
use fps_domain::{NodeId, ServerId};
use sqlx::MySqlPool;

use super::{from_naive, now_utc};

#[derive(Debug, Clone)]
pub struct Sample {
    pub node_id: NodeId,
    pub server_id: Option<ServerId>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<i64>,
    pub disk_available_bytes: Option<i64>,
    pub load_one: Option<f32>,
    pub running: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &MySqlPool,
    node_id: NodeId,
    server_id: Option<ServerId>,
    cpu_percent: Option<f64>,
    memory_bytes: Option<i64>,
    disk_available_bytes: Option<i64>,
    load_one: Option<f32>,
    running: Option<bool>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO resource_samples
            (node_id, server_id, cpu_percent, memory_bytes, disk_available_bytes, load_one, running, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(node_id.to_string())
    .bind(server_id.map(|s| s.to_string()))
    .bind(cpu_percent)
    .bind(memory_bytes)
    .bind(disk_available_bytes)
    .bind(load_one)
    .bind(running.map(i8::from))
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_for_node(
    pool: &MySqlPool,
    node_id: NodeId,
    limit: i64,
) -> Result<Vec<Sample>, sqlx::Error> {
    let rows: Vec<SampleRow> = sqlx::query_as(
        "SELECT node_id, server_id, cpu_percent, memory_bytes, disk_available_bytes, load_one, running, created_at
         FROM resource_samples WHERE node_id = ? AND server_id IS NULL
         ORDER BY id DESC LIMIT ?",
    )
    .bind(node_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    rows.into_iter().rev().map(TryInto::try_into).collect()
}

pub async fn list_for_server(
    pool: &MySqlPool,
    server_id: ServerId,
    limit: i64,
) -> Result<Vec<Sample>, sqlx::Error> {
    let rows: Vec<SampleRow> = sqlx::query_as(
        "SELECT node_id, server_id, cpu_percent, memory_bytes, disk_available_bytes, load_one, running, created_at
         FROM resource_samples WHERE server_id = ?
         ORDER BY id DESC LIMIT ?",
    )
    .bind(server_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    rows.into_iter().rev().map(TryInto::try_into).collect()
}

#[derive(sqlx::FromRow)]
struct SampleRow {
    node_id: String,
    server_id: Option<String>,
    cpu_percent: Option<f64>,
    memory_bytes: Option<i64>,
    disk_available_bytes: Option<i64>,
    load_one: Option<f32>,
    running: Option<i8>,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<SampleRow> for Sample {
    type Error = sqlx::Error;

    fn try_from(row: SampleRow) -> Result<Self, Self::Error> {
        Ok(Self {
            node_id: super::decode::parse_id(&row.node_id, "resource_samples.node_id")?,
            server_id: row
                .server_id
                .as_deref()
                .map(|s| super::decode::parse_id(s, "resource_samples.server_id"))
                .transpose()?,
            cpu_percent: row.cpu_percent,
            memory_bytes: row.memory_bytes,
            disk_available_bytes: row.disk_available_bytes,
            load_one: row.load_one,
            running: row.running.map(|v| v != 0),
            created_at: from_naive(row.created_at),
        })
    }
}
