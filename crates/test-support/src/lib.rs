//! Test helpers. This crate is not shipped in production binaries.

use std::env;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use tokio::sync::Mutex;

pub mod proxmox_fake;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

/// Connect to the shared MariaDB test database.
pub async fn test_pool() -> Option<(MySqlPool, String)> {
    let url = env::var("FPS_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "mysql://fps:local-dev-only@127.0.0.1:3306/fps_test".into());
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&url)
        .await
        .map_err(|e| {
            eprintln!("test database connect failed: {e}");
            e
        })
        .ok()?;
    Some((pool, "fps_test".into()))
}

/// Serialize mutating tests against the shared schema.
pub async fn lock_db() -> tokio::sync::MutexGuard<'static, ()> {
    DB_LOCK.lock().await
}

pub async fn reset_schema(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
        .execute(pool)
        .await?;
    for table in [
        "notifications",
        "schedules",
        "backups",
        "server_logs",
        "jobs",
        "allocations",
        "servers",
        "templates",
        "resource_samples",
        "update_history",
        "idempotency_keys",
        "audit_events",
        "node_enrollment_tokens",
        "nodes",
        "invitations",
        "sessions",
        "credentials",
        "users",
        "platform_settings",
        "singleton_locks",
    ] {
        let _ = sqlx::query(&format!("DELETE FROM `{table}`"))
            .execute(pool)
            .await;
    }
    sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
        .execute(pool)
        .await?;
    let _ = sqlx::query("INSERT IGNORE INTO singleton_locks (lock_name) VALUES ('setup')")
        .execute(pool)
        .await;
    Ok(())
}

pub async fn drop_database(_name: &str) {}
