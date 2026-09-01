use std::time::Duration;

use fps_domain::{BackupId, JobKind, ServerStatus};
use sqlx::MySqlPool;
use tracing::warn;

use crate::db::{backups, jobs, nodes, schedules, servers};

pub async fn run_loop(pool: MySqlPool) {
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    loop {
        tick.tick().await;
        if let Err(err) = tick_once(&pool).await {
            warn!(error = %err, "scheduler tick failed");
        }
    }
}

async fn tick_once(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    for rec in schedules::due(pool).await? {
        let server = servers::get(pool, rec.server_id).await?;
        let Some(server) = server else {
            schedules::mark_ran(pool, &rec).await?;
            continue;
        };
        let Some(node_id) = server.summary.node_id else {
            schedules::mark_ran(pool, &rec).await?;
            continue;
        };
        if nodes::get(pool, node_id)
            .await?
            .and_then(|n| n.revoked_at)
            .is_some()
        {
            schedules::mark_ran(pool, &rec).await?;
            continue;
        }
        match rec.action.as_str() {
            "backup" => {
                let backup_id = BackupId::new();
                backups::insert_pending(pool, backup_id, rec.server_id, node_id).await?;
                jobs::enqueue(
                    pool,
                    node_id,
                    Some(rec.server_id),
                    JobKind::Backup,
                    serde_json::json!({
                        "server_id": rec.server_id,
                        "container_name": server.summary.container_name,
                        "backup_id": backup_id,
                    }),
                )
                .await?;
            }
            "start" => {
                jobs::enqueue(
                    pool,
                    node_id,
                    Some(rec.server_id),
                    JobKind::Start,
                    serde_json::json!({
                        "server_id": rec.server_id,
                        "container_name": server.summary.container_name,
                    }),
                )
                .await?;
            }
            "stop" => {
                jobs::enqueue(
                    pool,
                    node_id,
                    Some(rec.server_id),
                    JobKind::Stop,
                    serde_json::json!({
                        "server_id": rec.server_id,
                        "container_name": server.summary.container_name,
                    }),
                )
                .await?;
            }
            _ => {}
        }
        let _ = ServerStatus::Pending;
        schedules::mark_ran(pool, &rec).await?;
    }
    Ok(())
}
