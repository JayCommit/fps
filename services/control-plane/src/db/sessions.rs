use chrono::{DateTime, Utc};
use fps_domain::{SessionId, UserId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct SessionRecord {
    pub id: SessionId,
    pub user_id: UserId,
    pub token_hash: String,
    pub csrf_token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub refresh_expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn insert(pool: &MySqlPool, rec: &NewSession) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sessions
            (id, user_id, token_hash, csrf_token_hash, refresh_token_hash, user_agent, ip, expires_at, refresh_expires_at, created_at, last_used_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(rec.id.to_string())
    .bind(rec.user_id.to_string())
    .bind(&rec.token_hash)
    .bind(&rec.csrf_token_hash)
    .bind(&rec.refresh_token_hash)
    .bind(&rec.user_agent)
    .bind(&rec.ip)
    .bind(rec.expires_at.naive_utc())
    .bind(rec.refresh_expires_at.naive_utc())
    .bind(now_utc())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub struct NewSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub token_hash: String,
    pub csrf_token_hash: String,
    pub refresh_token_hash: String,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
}

pub async fn find_by_token_hash(
    pool: &MySqlPool,
    token_hash: &str,
) -> Result<Option<SessionRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT id, user_id, token_hash, csrf_token_hash, refresh_token_hash, expires_at, refresh_expires_at, revoked_at
         FROM sessions WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    row.map(SessionRecord::try_from).transpose()
}

pub async fn find_by_refresh_hash(
    pool: &MySqlPool,
    refresh_hash: &str,
) -> Result<Option<SessionRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT id, user_id, token_hash, csrf_token_hash, refresh_token_hash, expires_at, refresh_expires_at, revoked_at
         FROM sessions WHERE refresh_token_hash = ?",
    )
    .bind(refresh_hash)
    .fetch_optional(pool)
    .await?;
    row.map(SessionRecord::try_from).transpose()
}

pub async fn revoke(pool: &MySqlPool, id: SessionId) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sessions SET revoked_at = ? WHERE id = ?")
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rotate(
    pool: &MySqlPool,
    id: SessionId,
    token_hash: &str,
    refresh_hash: &str,
    csrf_hash: &str,
    expires_at: DateTime<Utc>,
    refresh_expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE sessions SET token_hash = ?, refresh_token_hash = ?, csrf_token_hash = ?, expires_at = ?, refresh_expires_at = ?, last_used_at = ?
         WHERE id = ?",
    )
    .bind(token_hash)
    .bind(refresh_hash)
    .bind(csrf_hash)
    .bind(expires_at.naive_utc())
    .bind(refresh_expires_at.naive_utc())
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_for_user(
    pool: &MySqlPool,
    user_id: UserId,
) -> Result<Vec<SessionRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SessionRow>(
        "SELECT id, user_id, token_hash, csrf_token_hash, refresh_token_hash, expires_at, refresh_expires_at, revoked_at
         FROM sessions WHERE user_id = ? ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(SessionRecord::try_from).collect()
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    user_id: String,
    token_hash: String,
    csrf_token_hash: String,
    refresh_token_hash: Option<String>,
    expires_at: chrono::NaiveDateTime,
    refresh_expires_at: Option<chrono::NaiveDateTime>,
    revoked_at: Option<chrono::NaiveDateTime>,
}

impl TryFrom<SessionRow> for SessionRecord {
    type Error = sqlx::Error;

    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "sessions.id")?,
            user_id: parse_id(&row.user_id, "sessions.user_id")?,
            token_hash: row.token_hash,
            csrf_token_hash: row.csrf_token_hash,
            refresh_token_hash: row.refresh_token_hash,
            expires_at: from_naive(row.expires_at),
            refresh_expires_at: row.refresh_expires_at.map(from_naive),
            revoked_at: row.revoked_at.map(from_naive),
        })
    }
}
