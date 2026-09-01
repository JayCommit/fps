use chrono::{DateTime, Utc};
use fps_domain::{NodeId, ServerId};
use fps_protocol::LogChunk;
use sqlx::MySqlPool;

use super::{from_naive, now_utc};

pub struct LogRecord {
    pub stream: String,
    pub chunk: String,
    pub created_at: DateTime<Utc>,
}

pub async fn append_chunks(
    pool: &MySqlPool,
    node_id: NodeId,
    chunks: &[LogChunk],
) -> Result<(), sqlx::Error> {
    for chunk in chunks {
        sqlx::query(
            "INSERT INTO server_logs (server_id, node_id, stream, chunk, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(chunk.server_id.to_string())
        .bind(node_id.to_string())
        .bind(&chunk.stream)
        .bind(&chunk.text)
        .bind(now_utc())
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn recent(
    pool: &MySqlPool,
    server_id: ServerId,
    limit: i64,
) -> Result<Vec<LogRecord>, sqlx::Error> {
    let rows: Vec<(String, String, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT stream, chunk, created_at FROM server_logs WHERE server_id = ?
         ORDER BY id DESC LIMIT ?",
    )
    .bind(server_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .rev()
        .map(|(stream, chunk, created_at)| LogRecord {
            stream,
            chunk,
            created_at: from_naive(created_at),
        })
        .collect())
}
