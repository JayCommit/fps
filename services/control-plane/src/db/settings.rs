use serde_json::Value;
use sqlx::{MySql, MySqlPool, Transaction};

use super::now_utc;

pub async fn get_json(pool: &MySqlPool, key: &str) -> Result<Option<Value>, sqlx::Error> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT v_json FROM platform_settings WHERE k = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

pub async fn put_json(pool: &MySqlPool, key: &str, value: &Value) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    put_json_exec(&mut tx, key, value).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn put_json_exec(
    tx: &mut Transaction<'_, MySql>,
    key: &str,
    value: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO platform_settings (k, v_json, updated_at) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE v_json = VALUES(v_json), updated_at = VALUES(updated_at)",
    )
    .bind(key)
    .bind(value)
    .bind(now_utc())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn setup_completed(pool: &MySqlPool) -> Result<bool, sqlx::Error> {
    match get_json(pool, "setup_completed").await? {
        Some(Value::Bool(v)) => Ok(v),
        Some(other) => Ok(other.as_bool().unwrap_or(false)),
        None => Ok(false),
    }
}

pub async fn setup_completed_tx(tx: &mut Transaction<'_, MySql>) -> Result<bool, sqlx::Error> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT v_json FROM platform_settings WHERE k = ?")
            .bind("setup_completed")
            .fetch_optional(&mut **tx)
            .await?;
    Ok(match row.map(|r| r.0) {
        Some(Value::Bool(v)) => v,
        Some(other) => other.as_bool().unwrap_or(false),
        None => false,
    })
}
