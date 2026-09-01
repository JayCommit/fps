use fps_domain::{BackupId, BackupStatus, BackupSummary, NodeId, ServerId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub async fn insert_pending(
    pool: &MySqlPool,
    id: BackupId,
    server_id: ServerId,
    node_id: NodeId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO backups (id, server_id, node_id, status, created_at) VALUES (?, ?, ?, 'pending', ?)",
    )
    .bind(id.to_string())
    .bind(server_id.to_string())
    .bind(node_id.to_string())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn complete(
    pool: &MySqlPool,
    id: BackupId,
    success: bool,
    path: Option<&str>,
    bytes: Option<i64>,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let status = if success {
        BackupStatus::Succeeded
    } else {
        BackupStatus::Failed
    };
    sqlx::query(
        "UPDATE backups SET status = ?, archive_path = ?, size_bytes = ?, error = ?, completed_at = ? WHERE id = ?",
    )
    .bind(status.as_str())
    .bind(path)
    .bind(bytes)
    .bind(error)
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(
    pool: &MySqlPool,
    server_id: Option<ServerId>,
) -> Result<Vec<BackupSummary>, sqlx::Error> {
    let rows = if let Some(sid) = server_id {
        sqlx::query_as::<_, BackupRow>(
            "SELECT id, server_id, node_id, status, archive_path, size_bytes, error, created_at, completed_at
             FROM backups WHERE server_id = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(sid.to_string())
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, BackupRow>(
            "SELECT id, server_id, node_id, status, archive_path, size_bytes, error, created_at, completed_at
             FROM backups ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(pool)
        .await?
    };
    rows.into_iter().map(TryInto::try_into).collect()
}

#[derive(sqlx::FromRow)]
struct BackupRow {
    id: String,
    server_id: String,
    node_id: String,
    status: String,
    archive_path: Option<String>,
    size_bytes: Option<i64>,
    error: Option<String>,
    created_at: chrono::NaiveDateTime,
    completed_at: Option<chrono::NaiveDateTime>,
}

impl TryFrom<BackupRow> for BackupSummary {
    type Error = sqlx::Error;

    fn try_from(row: BackupRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "backups.id")?,
            server_id: parse_id(&row.server_id, "backups.server_id")?,
            node_id: parse_id(&row.node_id, "backups.node_id")?,
            status: BackupStatus::parse(&row.status),
            archive_path: row.archive_path,
            size_bytes: row.size_bytes,
            error: row.error,
            created_at: from_naive(row.created_at),
            completed_at: row.completed_at.map(from_naive),
        })
    }
}
