use chrono::{DateTime, Duration, Utc};
use fps_auth::hash_token;
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;

use super::now_utc;

pub struct StoredResponse {
    pub status: i32,
    pub body: Vec<u8>,
}

pub async fn lookup(
    pool: &MySqlPool,
    actor_hash: &str,
    key: &str,
    method: &str,
    path: &str,
    request_hash: &str,
) -> Result<Option<Result<StoredResponse, ()>>, sqlx::Error> {
    let key_hash = hash_token(key);
    let row: Option<(String, i32, Vec<u8>)> = sqlx::query_as(
        "SELECT request_hash, status, response_body FROM idempotency_keys
         WHERE actor_hash = ? AND key_hash = ? AND method = ? AND path = ?
           AND expires_at > ?",
    )
    .bind(actor_hash)
    .bind(&key_hash)
    .bind(method)
    .bind(path)
    .bind(now_utc())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(stored_req, status, body)| {
        if stored_req == request_hash {
            Ok(StoredResponse { status, body })
        } else {
            Err(())
        }
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn store(
    pool: &MySqlPool,
    actor_hash: &str,
    key: &str,
    method: &str,
    path: &str,
    request_hash: &str,
    status: i32,
    body: &[u8],
) -> Result<(), sqlx::Error> {
    let expires = Utc::now() + Duration::hours(24);
    sqlx::query(
        "INSERT INTO idempotency_keys
            (id, actor_hash, key_hash, method, path, request_hash, status, response_body, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::now_v7().to_string())
    .bind(actor_hash)
    .bind(hash_token(key))
    .bind(method)
    .bind(path)
    .bind(request_hash)
    .bind(status)
    .bind(body)
    .bind(now_utc())
    .bind(expires.naive_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub fn request_hash(body: &str) -> String {
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    hex::encode(h.finalize())
}

pub fn actor_hash(user_id: &str) -> String {
    hash_token(user_id)
}

#[allow(dead_code)]
pub fn _expires_uses(ts: DateTime<Utc>) -> DateTime<Utc> {
    ts
}
