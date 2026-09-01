use chrono::{DateTime, Utc};
use fps_domain::{InvitationId, Role, UserId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct InvitationRecord {
    pub id: InvitationId,
    pub email: String,
    pub role: Role,
    pub token_hash: String,
    pub invited_by: UserId,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &MySqlPool,
    id: InvitationId,
    email: &str,
    role: Role,
    token_hash: &str,
    invited_by: UserId,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO invitations (id, email, role, token_hash, invited_by, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(email)
    .bind(role.as_str())
    .bind(token_hash)
    .bind(invited_by.to_string())
    .bind(expires_at.naive_utc())
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &MySqlPool) -> Result<Vec<InvitationRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, InvitationRow>(
        "SELECT id, email, role, token_hash, invited_by, expires_at, accepted_at, created_at
         FROM invitations ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(InvitationRecord::try_from).collect()
}

pub async fn find_by_token_hash(
    pool: &MySqlPool,
    token_hash: &str,
) -> Result<Option<InvitationRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, InvitationRow>(
        "SELECT id, email, role, token_hash, invited_by, expires_at, accepted_at, created_at
         FROM invitations WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    row.map(InvitationRecord::try_from).transpose()
}

pub async fn mark_accepted(pool: &MySqlPool, id: InvitationId) -> Result<u64, sqlx::Error> {
    let res =
        sqlx::query("UPDATE invitations SET accepted_at = ? WHERE id = ? AND accepted_at IS NULL")
            .bind(now_utc())
            .bind(id.to_string())
            .execute(pool)
            .await?;
    Ok(res.rows_affected())
}

#[derive(sqlx::FromRow)]
struct InvitationRow {
    id: String,
    email: String,
    role: String,
    token_hash: String,
    invited_by: String,
    expires_at: chrono::NaiveDateTime,
    accepted_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<InvitationRow> for InvitationRecord {
    type Error = sqlx::Error;

    fn try_from(row: InvitationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "invitations.id")?,
            email: row.email,
            role: Role::parse(&row.role).unwrap_or(Role::Viewer),
            token_hash: row.token_hash,
            invited_by: parse_id(&row.invited_by, "invitations.invited_by")?,
            expires_at: from_naive(row.expires_at),
            accepted_at: row.accepted_at.map(from_naive),
            created_at: from_naive(row.created_at),
        })
    }
}
