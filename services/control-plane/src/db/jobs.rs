use chrono::{DateTime, Utc};
use fps_domain::{JobId, JobKind, JobStatus, NodeId, ServerId};
use fps_protocol::{JobInstruction, JobResult};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct JobRecord {
    pub id: JobId,
    pub node_id: NodeId,
    pub server_id: Option<ServerId>,
    pub kind: JobKind,
    pub status: JobStatus,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

pub async fn enqueue(
    pool: &MySqlPool,
    node_id: NodeId,
    server_id: Option<ServerId>,
    kind: JobKind,
    payload: serde_json::Value,
) -> Result<JobId, sqlx::Error> {
    let id = JobId::new();
    sqlx::query(
        "INSERT INTO jobs (id, node_id, server_id, kind, status, payload_json, created_at)
         VALUES (?, ?, ?, ?, 'queued', ?, ?)",
    )
    .bind(id.to_string())
    .bind(node_id.to_string())
    .bind(server_id.map(|s| s.to_string()))
    .bind(kind.as_str())
    .bind(payload.to_string())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn claim_for_node(
    pool: &MySqlPool,
    node_id: NodeId,
    limit: i64,
) -> Result<Vec<JobInstruction>, sqlx::Error> {
    let rows: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT id, kind, payload_json FROM jobs
         WHERE node_id = ? AND status = 'queued'
         ORDER BY created_at ASC LIMIT ?",
    )
    .bind(node_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for (id, kind, payload) in rows {
        let updated = sqlx::query(
            "UPDATE jobs SET status = 'dispatched', dispatched_at = ? WHERE id = ? AND status = 'queued'",
        )
        .bind(now_utc())
        .bind(&id)
        .execute(pool)
        .await?;
        if updated.rows_affected() != 1 {
            continue;
        }
        out.push(JobInstruction {
            id: parse_id(&id, "jobs.id")?,
            kind: JobKind::parse(&kind),
            payload,
        });
    }
    Ok(out)
}

pub async fn complete(
    pool: &MySqlPool,
    result: &JobResult,
) -> Result<Option<JobRecord>, sqlx::Error> {
    let status = if result.success {
        JobStatus::Succeeded
    } else {
        JobStatus::Failed
    };
    let body = serde_json::to_value(result).unwrap_or(serde_json::json!({}));
    sqlx::query("UPDATE jobs SET status = ?, result_json = ?, completed_at = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(body.to_string())
        .bind(now_utc())
        .bind(result.id.to_string())
        .execute(pool)
        .await?;
    get(pool, result.id).await
}

pub async fn get(pool: &MySqlPool, id: JobId) -> Result<Option<JobRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, JobRow>(
        "SELECT id, node_id, server_id, kind, status, payload_json, result_json, created_at FROM jobs WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(JobRecord::try_from).transpose()
}

pub async fn list_for_server(
    pool: &MySqlPool,
    server_id: ServerId,
) -> Result<Vec<JobRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, JobRow>(
        "SELECT id, node_id, server_id, kind, status, payload_json, result_json, created_at
         FROM jobs WHERE server_id = ? ORDER BY created_at DESC LIMIT 100",
    )
    .bind(server_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(JobRecord::try_from).collect()
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: String,
    node_id: String,
    server_id: Option<String>,
    kind: String,
    status: String,
    payload_json: serde_json::Value,
    result_json: Option<serde_json::Value>,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<JobRow> for JobRecord {
    type Error = sqlx::Error;

    fn try_from(row: JobRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "jobs.id")?,
            node_id: parse_id(&row.node_id, "jobs.node_id")?,
            server_id: row
                .server_id
                .map(|s| parse_id(&s, "jobs.server_id"))
                .transpose()?,
            kind: JobKind::parse(&row.kind),
            status: JobStatus::parse(&row.status),
            payload: row.payload_json,
            result: row.result_json,
            created_at: from_naive(row.created_at),
        })
    }
}
