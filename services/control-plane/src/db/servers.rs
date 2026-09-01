use fps_domain::{AllocationId, NodeId, ServerId, ServerStatus, ServerSummary, TemplateId, UserId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct ServerRecord {
    pub summary: ServerSummary,
    pub environment_json: String,
    pub container_id: Option<String>,
    pub files_json: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &MySqlPool,
    id: ServerId,
    name: &str,
    template_id: TemplateId,
    node_id: NodeId,
    allocation_id: AllocationId,
    environment_json: &str,
    memory_mb: i32,
    cpu_shares: i32,
    container_name: &str,
    created_by: UserId,
) -> Result<(), sqlx::Error> {
    let now = now_utc();
    sqlx::query(
        "INSERT INTO servers
            (id, name, template_id, node_id, allocation_id, status, environment_json, memory_mb,
             cpu_shares, container_name, created_by, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(name)
    .bind(template_id.to_string())
    .bind(node_id.to_string())
    .bind(allocation_id.to_string())
    .bind(environment_json)
    .bind(memory_mb)
    .bind(cpu_shares)
    .bind(container_name)
    .bind(created_by.to_string())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &MySqlPool) -> Result<Vec<ServerRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ServerRow>(SERVER_COLS)
        .fetch_all(pool)
        .await?;
    rows.into_iter().map(ServerRecord::try_from).collect()
}

pub async fn get(pool: &MySqlPool, id: ServerId) -> Result<Option<ServerRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, ServerRow>(&format!("{SERVER_COLS} WHERE id = ?"))
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    row.map(ServerRecord::try_from).transpose()
}

pub async fn set_status(
    pool: &MySqlPool,
    id: ServerId,
    status: ServerStatus,
    last_error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE servers SET status = ?, last_error = ?, updated_at = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(last_error)
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_container(
    pool: &MySqlPool,
    id: ServerId,
    container_id: Option<&str>,
    status: ServerStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE servers SET container_id = ?, status = ?, last_error = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(container_id)
    .bind(status.as_str())
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_files(
    pool: &MySqlPool,
    id: ServerId,
    files: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE servers SET files_json = ?, updated_at = ? WHERE id = ?")
        .bind(files.to_string())
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn counts(pool: &MySqlPool) -> Result<(i64, i64), sqlx::Error> {
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM servers")
        .fetch_one(pool)
        .await?;
    let (running,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM servers WHERE status = 'running'")
            .fetch_one(pool)
            .await?;
    Ok((total, running))
}

const SERVER_COLS: &str = "SELECT id, name, template_id, node_id, allocation_id, status, environment_json,
        memory_mb, cpu_shares, container_name, container_id, last_error, files_json, created_at, updated_at
 FROM servers";

#[derive(sqlx::FromRow)]
struct ServerRow {
    id: String,
    name: String,
    template_id: String,
    node_id: Option<String>,
    allocation_id: Option<String>,
    status: String,
    environment_json: serde_json::Value,
    memory_mb: i32,
    cpu_shares: i32,
    container_name: Option<String>,
    container_id: Option<String>,
    last_error: Option<String>,
    files_json: Option<serde_json::Value>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl TryFrom<ServerRow> for ServerRecord {
    type Error = sqlx::Error;

    fn try_from(row: ServerRow) -> Result<Self, Self::Error> {
        Ok(Self {
            summary: ServerSummary {
                id: parse_id(&row.id, "servers.id")?,
                name: row.name,
                template_id: parse_id(&row.template_id, "servers.template_id")?,
                node_id: row
                    .node_id
                    .as_deref()
                    .map(|s| parse_id(s, "servers.node_id"))
                    .transpose()?,
                allocation_id: row
                    .allocation_id
                    .as_deref()
                    .map(|s| parse_id(s, "servers.allocation_id"))
                    .transpose()?,
                status: ServerStatus::parse(&row.status),
                memory_mb: row.memory_mb,
                cpu_shares: row.cpu_shares,
                container_name: row.container_name,
                last_error: row.last_error,
                created_at: from_naive(row.created_at),
                updated_at: from_naive(row.updated_at),
            },
            environment_json: row.environment_json.to_string(),
            container_id: row.container_id,
            files_json: row.files_json.map(|v| v.to_string()),
        })
    }
}
