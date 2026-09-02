pub mod allocations;
pub mod audit;
pub mod backups;
pub mod decode;
pub mod idempotency;
pub mod invitations;
pub mod jobs;
pub mod logs;
pub mod metrics;
pub mod nodes;
pub mod notifications;
pub mod schedules;
pub mod servers;
pub mod sessions;
pub mod settings;
pub mod templates;
pub mod users;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

pub async fn connect(database_url: &str) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?)
}

pub async fn migrate(pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub fn now_utc() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

pub fn from_naive(ts: chrono::NaiveDateTime) -> chrono::DateTime<chrono::Utc> {
    ts.and_utc()
}
