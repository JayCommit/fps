use chrono::{DateTime, Utc};
use fps_domain::{Role, UserId, UserStatus, UserSummary};
use sqlx::{MySql, MySqlPool, Transaction};

use super::decode::{column_decode, parse_id};
use super::{from_naive, now_utc};

pub struct UserRecord {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub totp_secret_encrypted: Option<String>,
    pub totp_pending_encrypted: Option<String>,
    pub totp_enabled: bool,
    pub recovery_hashes_json: String,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
}

impl UserRecord {
    pub fn summary(&self) -> UserSummary {
        UserSummary {
            id: self.id,
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            role: self.role,
            totp_enabled: self.totp_enabled,
            status: self.status,
            created_at: self.created_at,
        }
    }

    pub fn recovery_hashes(&self) -> Vec<String> {
        serde_json::from_str(&self.recovery_hashes_json).unwrap_or_default()
    }
}

pub async fn count_users(pool: &MySqlPool) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

pub async fn lock_setup(tx: &mut Transaction<'_, MySql>) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT lock_name FROM singleton_locks WHERE lock_name = 'setup' FOR UPDATE")
        .fetch_one(&mut **tx)
        .await?;
    Ok(())
}

pub async fn insert_owner_tx(
    tx: &mut Transaction<'_, MySql>,
    id: UserId,
    email: &str,
    display_name: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    insert_user_tx(tx, id, email, display_name, Role::Owner, password_hash).await
}

pub async fn insert_user_tx(
    tx: &mut Transaction<'_, MySql>,
    id: UserId,
    email: &str,
    display_name: &str,
    role: Role,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    let now = now_utc();
    sqlx::query(
        "INSERT INTO users (id, email, display_name, role, totp_enabled, recovery_hashes_json, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, 0, JSON_ARRAY(), 'active', ?, ?)",
    )
    .bind(id.to_string())
    .bind(email)
    .bind(display_name)
    .bind(role.as_str())
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO credentials (id, user_id, kind, secret_hash, created_at) VALUES (?, ?, 'password', ?, ?)",
    )
    .bind(uuid::Uuid::now_v7().to_string())
    .bind(id.to_string())
    .bind(password_hash)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn insert_user(
    pool: &MySqlPool,
    id: UserId,
    email: &str,
    display_name: &str,
    role: Role,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    insert_user_tx(&mut tx, id, email, display_name, role, password_hash).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn find_by_email(
    pool: &MySqlPool,
    email: &str,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, display_name, role, totp_secret_encrypted, totp_pending_encrypted, totp_enabled, recovery_hashes_json, status, created_at
         FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    row.map(UserRecord::try_from).transpose()
}

pub async fn find_by_id(pool: &MySqlPool, id: UserId) -> Result<Option<UserRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, display_name, role, totp_secret_encrypted, totp_pending_encrypted, totp_enabled, recovery_hashes_json, status, created_at
         FROM users WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(UserRecord::try_from).transpose()
}

pub async fn password_hash(
    pool: &MySqlPool,
    user_id: UserId,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT secret_hash FROM credentials WHERE user_id = ? AND kind = 'password' LIMIT 1",
    )
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn set_totp_pending(
    pool: &MySqlPool,
    user_id: UserId,
    encrypted_pending: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET totp_pending_encrypted = ?, updated_at = ? WHERE id = ?")
        .bind(encrypted_pending)
        .bind(now_utc())
        .bind(user_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_totp(
    pool: &MySqlPool,
    user_id: UserId,
    encrypted_secret: &str,
    recovery_hashes: &[String],
) -> Result<(), sqlx::Error> {
    let hashes = serde_json::to_string(recovery_hashes).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "UPDATE users SET totp_secret_encrypted = ?, totp_pending_encrypted = NULL, totp_enabled = 1, recovery_hashes_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(encrypted_secret)
    .bind(hashes)
    .bind(now_utc())
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn replace_recovery_hashes(
    pool: &MySqlPool,
    user_id: UserId,
    hashes: &[String],
) -> Result<(), sqlx::Error> {
    let encoded = serde_json::to_string(hashes).unwrap_or_else(|_| "[]".into());
    sqlx::query("UPDATE users SET recovery_hashes_json = ?, updated_at = ? WHERE id = ?")
        .bind(encoded)
        .bind(now_utc())
        .bind(user_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list(pool: &MySqlPool) -> Result<Vec<UserRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, display_name, role, totp_secret_encrypted, totp_pending_encrypted, totp_enabled, recovery_hashes_json, status, created_at
         FROM users ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(UserRecord::try_from).collect()
}

pub async fn set_status(
    pool: &MySqlPool,
    id: UserId,
    status: UserStatus,
) -> Result<(), sqlx::Error> {
    let disabled_at = if matches!(status, UserStatus::Disabled) {
        Some(now_utc())
    } else {
        None
    };
    let label = match status {
        UserStatus::Active => "active",
        UserStatus::Disabled => "disabled",
    };
    sqlx::query("UPDATE users SET status = ?, disabled_at = ?, updated_at = ? WHERE id = ?")
        .bind(label)
        .bind(disabled_at)
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_role(pool: &MySqlPool, id: UserId, role: Role) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET role = ?, updated_at = ? WHERE id = ?")
        .bind(role.as_str())
        .bind(now_utc())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count_owners(pool: &MySqlPool) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'owner' AND status = 'active'")
            .fetch_one(pool)
            .await?;
    Ok(n)
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    email: String,
    display_name: String,
    role: String,
    totp_secret_encrypted: Option<String>,
    totp_pending_encrypted: Option<String>,
    totp_enabled: i8,
    recovery_hashes_json: serde_json::Value,
    status: String,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<UserRow> for UserRecord {
    type Error = sqlx::Error;

    fn try_from(row: UserRow) -> Result<Self, Self::Error> {
        let id = parse_id(&row.id, "users.id")?;
        let role = Role::parse(&row.role).map_err(|e| column_decode("users.role", e))?;
        let status = if row.status == "disabled" {
            UserStatus::Disabled
        } else {
            UserStatus::Active
        };
        Ok(Self {
            id,
            email: row.email,
            display_name: row.display_name,
            role,
            totp_secret_encrypted: row.totp_secret_encrypted,
            totp_pending_encrypted: row.totp_pending_encrypted,
            totp_enabled: row.totp_enabled != 0,
            recovery_hashes_json: row.recovery_hashes_json.to_string(),
            status,
            created_at: from_naive(row.created_at),
        })
    }
}
