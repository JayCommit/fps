use chrono::{DateTime, Utc};
use fps_domain::{AllocationId, NodeId, ServerId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub const PORT_RANGE_START: i32 = 25000;
pub const PORT_RANGE_END: i32 = 25999;

pub struct AllocationRecord {
    pub id: AllocationId,
    pub node_id: NodeId,
    pub ip: String,
    pub port: i32,
    pub protocol: String,
    pub assigned_server_id: Option<ServerId>,
    pub created_at: DateTime<Utc>,
}

pub async fn allocate_next(
    pool: &MySqlPool,
    node_id: NodeId,
    protocol: &str,
) -> Result<AllocationRecord, sqlx::Error> {
    let used: Vec<(i32,)> = sqlx::query_as(
        "SELECT port FROM allocations WHERE node_id = ? AND protocol = ? ORDER BY port",
    )
    .bind(node_id.to_string())
    .bind(protocol)
    .fetch_all(pool)
    .await?;
    let used: std::collections::HashSet<i32> = used.into_iter().map(|r| r.0).collect();
    let port = (PORT_RANGE_START..=PORT_RANGE_END)
        .find(|p| !used.contains(p))
        .ok_or_else(|| sqlx::Error::Protocol("no free ports on node".into()))?;
    let id = AllocationId::new();
    let now = now_utc();
    sqlx::query(
        "INSERT INTO allocations (id, node_id, ip, port, protocol, assigned_server_id, created_at)
         VALUES (?, ?, '0.0.0.0', ?, ?, NULL, ?)",
    )
    .bind(id.to_string())
    .bind(node_id.to_string())
    .bind(port)
    .bind(protocol)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(AllocationRecord {
        id,
        node_id,
        ip: "0.0.0.0".into(),
        port,
        protocol: protocol.into(),
        assigned_server_id: None,
        created_at: from_naive(now),
    })
}

pub async fn assign_server(
    pool: &MySqlPool,
    id: AllocationId,
    server_id: ServerId,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE allocations SET assigned_server_id = ? WHERE id = ?")
        .bind(server_id.to_string())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

#[allow(clippy::type_complexity)]
pub async fn get(
    pool: &MySqlPool,
    id: AllocationId,
) -> Result<Option<AllocationRecord>, sqlx::Error> {
    let row: Option<(String, String, String, i32, String, Option<String>, chrono::NaiveDateTime)> =
        sqlx::query_as(
            "SELECT id, node_id, ip, port, protocol, assigned_server_id, created_at FROM allocations WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    row.map(|(id, node_id, ip, port, protocol, assigned, created_at)| {
        Ok(AllocationRecord {
            id: parse_id(&id, "allocations.id")?,
            node_id: parse_id(&node_id, "allocations.node_id")?,
            ip,
            port,
            protocol,
            assigned_server_id: assigned
                .map(|s| parse_id(&s, "allocations.assigned_server_id"))
                .transpose()?,
            created_at: from_naive(created_at),
        })
    })
    .transpose()
}
