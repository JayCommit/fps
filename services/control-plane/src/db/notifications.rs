use chrono::{DateTime, Utc};
use fps_domain::{NotificationId, UserId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct NotificationRecord {
    pub id: NotificationId,
    pub user_id: Option<UserId>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &MySqlPool,
    kind: &str,
    title: &str,
    body: &str,
) -> Result<NotificationId, sqlx::Error> {
    let id = NotificationId::new();
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, body, created_at) VALUES (?, NULL, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(kind)
    .bind(title)
    .bind(body)
    .bind(now_utc())
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn list(pool: &MySqlPool) -> Result<Vec<NotificationRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, NotifRow>(
        "SELECT id, user_id, kind, title, body, read_at, created_at FROM notifications
         ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

pub async fn mark_read(pool: &MySqlPool, id: NotificationId) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notifications SET read_at = ? WHERE id = ?")
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct NotifRow {
    id: String,
    user_id: Option<String>,
    kind: String,
    title: String,
    body: String,
    read_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<NotifRow> for NotificationRecord {
    type Error = sqlx::Error;

    fn try_from(row: NotifRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&row.id, "notifications.id")?,
            user_id: row
                .user_id
                .map(|s| parse_id(&s, "notifications.user_id"))
                .transpose()?,
            kind: row.kind,
            title: row.title,
            body: row.body,
            read_at: row.read_at.map(from_naive),
            created_at: from_naive(row.created_at),
        })
    }
}
