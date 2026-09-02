use fps_domain::{AddonInstallId, AddonInstallStatus, JobId, ServerAddonSummary, ServerId};
use fps_templates::AddonSpec;
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct AddonRecord {
    pub summary: ServerAddonSummary,
    pub spec_json: String,
}

pub async fn list_for_server(
    pool: &MySqlPool,
    server_id: ServerId,
) -> Result<Vec<AddonRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, AddonRow>(
        "SELECT id, server_id, addon_slug, addon_name, version_label, status, tracked_paths_json,
                spec_json, job_id, error, installed_at, created_at, updated_at
         FROM server_addons WHERE server_id = ? ORDER BY addon_name",
    )
    .bind(server_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(AddonRecord::try_from).collect()
}

pub async fn get_for_server(
    pool: &MySqlPool,
    server_id: ServerId,
    slug: &str,
) -> Result<Option<AddonRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, AddonRow>(
        "SELECT id, server_id, addon_slug, addon_name, version_label, status, tracked_paths_json,
                spec_json, job_id, error, installed_at, created_at, updated_at
         FROM server_addons WHERE server_id = ? AND addon_slug = ?",
    )
    .bind(server_id.to_string())
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    row.map(AddonRecord::try_from).transpose()
}

pub async fn upsert_queued(
    pool: &MySqlPool,
    server_id: ServerId,
    spec: &AddonSpec,
    job_id: JobId,
    status: AddonInstallStatus,
) -> Result<AddonInstallId, sqlx::Error> {
    if let Some(existing) = get_for_server(pool, server_id, &spec.slug).await? {
        sqlx::query(
            "UPDATE server_addons
             SET addon_name = ?, version_label = ?, status = ?, spec_json = ?, job_id = ?,
                 error = NULL, updated_at = ?
             WHERE id = ?",
        )
        .bind(&spec.name)
        .bind(&spec.version_label)
        .bind(status.as_str())
        .bind(serde_json::to_string(spec).unwrap_or_else(|_| "{}".into()))
        .bind(job_id.to_string())
        .bind(now_utc())
        .bind(existing.summary.id.to_string())
        .execute(pool)
        .await?;
        return Ok(existing.summary.id);
    }
    let id = AddonInstallId::new();
    let now = now_utc();
    sqlx::query(
        "INSERT INTO server_addons
            (id, server_id, addon_slug, addon_name, version_label, status, tracked_paths_json,
             spec_json, job_id, error, installed_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, '[]', ?, ?, NULL, NULL, ?, ?)",
    )
    .bind(id.to_string())
    .bind(server_id.to_string())
    .bind(&spec.slug)
    .bind(&spec.name)
    .bind(&spec.version_label)
    .bind(status.as_str())
    .bind(serde_json::to_string(spec).unwrap_or_else(|_| "{}".into()))
    .bind(job_id.to_string())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn mark_installed(
    pool: &MySqlPool,
    id: AddonInstallId,
    tracked_paths: &[String],
) -> Result<(), sqlx::Error> {
    let paths = serde_json::to_string(tracked_paths).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "UPDATE server_addons
         SET status = 'installed', tracked_paths_json = ?, error = NULL, installed_at = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(paths)
    .bind(now_utc())
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(
    pool: &MySqlPool,
    id: AddonInstallId,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE server_addons SET status = 'failed', error = ?, updated_at = ? WHERE id = ?",
    )
    .bind(error)
    .bind(now_utc())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &MySqlPool, id: AddonInstallId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM server_addons WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct AddonRow {
    id: String,
    server_id: String,
    addon_slug: String,
    addon_name: String,
    version_label: String,
    status: String,
    tracked_paths_json: serde_json::Value,
    spec_json: serde_json::Value,
    job_id: Option<String>,
    error: Option<String>,
    installed_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl TryFrom<AddonRow> for AddonRecord {
    type Error = sqlx::Error;

    fn try_from(row: AddonRow) -> Result<Self, Self::Error> {
        let tracked_paths: Vec<String> =
            serde_json::from_value(row.tracked_paths_json.clone()).unwrap_or_default();
        Ok(Self {
            summary: ServerAddonSummary {
                id: parse_id(&row.id, "server_addons.id")?,
                server_id: parse_id(&row.server_id, "server_addons.server_id")?,
                addon_slug: row.addon_slug,
                addon_name: row.addon_name,
                version_label: row.version_label,
                status: AddonInstallStatus::parse(&row.status),
                tracked_paths,
                job_id: row
                    .job_id
                    .map(|s| parse_id(&s, "server_addons.job_id"))
                    .transpose()?,
                error: row.error,
                installed_at: row.installed_at.map(from_naive),
                created_at: from_naive(row.created_at),
                updated_at: from_naive(row.updated_at),
            },
            spec_json: row.spec_json.to_string(),
        })
    }
}

pub fn parse_spec(json: &str) -> Option<AddonSpec> {
    serde_json::from_str(json).ok()
}
