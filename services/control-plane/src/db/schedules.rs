use chrono::{DateTime, Duration, Utc};
use fps_domain::{ScheduleId, ServerId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct ScheduleRecord {
    pub id: ScheduleId,
    pub server_id: ServerId,
    pub name: String,
    pub interval_seconds: i32,
    pub action: String,
    pub enabled: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &MySqlPool,
    server_id: ServerId,
    name: &str,
    interval_seconds: i32,
    action: &str,
) -> Result<ScheduleId, sqlx::Error> {
    let id = ScheduleId::new();
    let next = Utc::now() + Duration::seconds(interval_seconds as i64);
    sqlx::query(
        "INSERT INTO schedules (id, server_id, name, interval_seconds, action, enabled, next_run_at, created_at)
         VALUES (?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(id.to_string())
    .bind(server_id.to_string())
    .bind(name)
    .bind(interval_seconds)
    .bind(action)
    .bind(next.naive_utc())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn list(
    pool: &MySqlPool,
    server_id: Option<ServerId>,
) -> Result<Vec<ScheduleRecord>, sqlx::Error> {
    let rows = if let Some(sid) = server_id {
        sqlx::query_as::<_, ScheduleRow>(
            "SELECT id, server_id, name, interval_seconds, action, enabled, last_run_at, next_run_at, created_at
             FROM schedules WHERE server_id = ? ORDER BY created_at DESC",
        )
        .bind(sid.to_string())
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, ScheduleRow>(
            "SELECT id, server_id, name, interval_seconds, action, enabled, last_run_at, next_run_at, created_at
             FROM schedules ORDER BY created_at DESC LIMIT 200",
        )
        .fetch_all(pool)
        .await?
    };
    rows.into_iter().map(TryInto::try_into).collect()
}

pub async fn set_enabled(
    pool: &MySqlPool,
    id: ScheduleId,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE schedules SET enabled = ? WHERE id = ?")
        .bind(enabled as i8)
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn due(pool: &MySqlPool) -> Result<Vec<ScheduleRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ScheduleRow>(
        "SELECT id, server_id, name, interval_seconds, action, enabled, last_run_at, next_run_at, created_at
         FROM schedules WHERE enabled = 1 AND next_run_at <= ? LIMIT 50",
    )
    .bind(now_utc())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

pub async fn mark_ran(pool: &MySqlPool, rec: &ScheduleRecord) -> Result<(), sqlx::Error> {
    let next = Utc::now() + Duration::seconds(rec.interval_seconds as i64);
    sqlx::query("UPDATE schedules SET last_run_at = ?, next_run_at = ? WHERE id = ?")
        .bind(now_utc())
        .bind(next.naive_utc())
        .bind(rec.id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ScheduleRow {
    id: String,
    server_id: String,
    name: String,
    interval_seconds: i32,
    action: String,
    enabled: i8,
    last_run_at: Option<chrono::NaiveDateTime>,
    next_run_at: chrono::NaiveDateTime,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<ScheduleRow> for ScheduleRecord {
    type Error = sqlx::Error;

    fn try_from(row: ScheduleRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "schedules.id")?,
            server_id: parse_id(&row.server_id, "schedules.server_id")?,
            name: row.name,
            interval_seconds: row.interval_seconds,
            action: row.action,
            enabled: row.enabled != 0,
            last_run_at: row.last_run_at.map(from_naive),
            next_run_at: from_naive(row.next_run_at),
            created_at: from_naive(row.created_at),
        })
    }
}
