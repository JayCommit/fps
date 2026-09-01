use fps_domain::{AuditEventId, NodeId, RequestId, UserId};
use serde_json::Value;
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

#[allow(clippy::too_many_arguments)]
pub async fn record(
    pool: &MySqlPool,
    actor_user: Option<UserId>,
    actor_node: Option<NodeId>,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    ip: Option<&str>,
    request_id: Option<RequestId>,
    details: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events
            (id, actor_user_id, actor_node_id, action, resource_type, resource_id, ip, request_id, details_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(AuditEventId::new().to_string())
    .bind(actor_user.map(|id| id.to_string()))
    .bind(actor_node.map(|id| id.to_string()))
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(ip)
    .bind(request_id.map(|id| id.to_string()))
    .bind(details.to_string())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub struct AuditRecord {
    pub id: AuditEventId,
    pub actor_user_id: Option<UserId>,
    pub actor_node_id: Option<NodeId>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub details: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list(pool: &MySqlPool, limit: i64) -> Result<Vec<AuditRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT id, actor_user_id, actor_node_id, action, resource_type, resource_id, details_json, created_at
         FROM audit_events ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(AuditRecord::try_from).collect()
}

#[derive(sqlx::FromRow)]
struct AuditRow {
    id: String,
    actor_user_id: Option<String>,
    actor_node_id: Option<String>,
    action: String,
    resource_type: String,
    resource_id: Option<String>,
    details_json: serde_json::Value,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<AuditRow> for AuditRecord {
    type Error = sqlx::Error;

    fn try_from(row: AuditRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "audit_events.id")?,
            actor_user_id: row
                .actor_user_id
                .map(|s| parse_id(&s, "audit_events.actor_user_id"))
                .transpose()?,
            actor_node_id: row
                .actor_node_id
                .map(|s| parse_id(&s, "audit_events.actor_node_id"))
                .transpose()?,
            action: row.action,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            details: row.details_json,
            created_at: from_naive(row.created_at),
        })
    }
}
