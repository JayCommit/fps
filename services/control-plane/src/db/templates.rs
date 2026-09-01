use fps_domain::{PortMapping, TemplateId, TemplateSource, TemplateSummary};
use fps_templates::NativeTemplate;
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

pub struct TemplateRecord {
    pub summary: TemplateSummary,
    pub env_json: String,
    pub body_json: String,
    pub startup_command: Option<String>,
    pub volume_path: String,
    pub cpu_shares: i32,
}

pub async fn ensure_catalogue(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    for native in [
        fps_templates::http_echo_catalogue(),
        fps_templates::minecraft_catalogue(),
    ] {
        if find_by_slug(pool, &native.slug).await?.is_none() {
            insert_native(pool, &native).await?;
        }
    }
    Ok(())
}

pub async fn insert_native(
    pool: &MySqlPool,
    native: &NativeTemplate,
) -> Result<TemplateId, sqlx::Error> {
    let id = TemplateId::new();
    let now = now_utc();
    let env = serde_json::to_string(&native.environment).unwrap_or_else(|_| "{}".into());
    let ports = serde_json::to_string(&native.ports).unwrap_or_else(|_| "[]".into());
    let body = serde_json::to_string(native).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO templates
            (id, name, slug, description, docker_image, startup_command, env_json, ports_json,
             memory_mb, cpu_shares, volume_path, source, body_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'native', ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(&native.name)
    .bind(&native.slug)
    .bind(&native.description)
    .bind(&native.docker_image)
    .bind(&native.startup)
    .bind(env)
    .bind(ports)
    .bind(native.memory_mb as i32)
    .bind(native.cpu_shares as i32)
    .bind(&native.volume_path)
    .bind(body)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn insert_imported(
    pool: &MySqlPool,
    native: &NativeTemplate,
) -> Result<TemplateId, sqlx::Error> {
    let id = insert_native(pool, native).await?;
    sqlx::query("UPDATE templates SET source = 'egg_import' WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn list(pool: &MySqlPool) -> Result<Vec<TemplateRecord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, name, slug, description, docker_image, startup_command, env_json, ports_json,
                memory_mb, cpu_shares, volume_path, source, body_json, created_at
         FROM templates ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(TemplateRecord::try_from).collect()
}

pub async fn get(pool: &MySqlPool, id: TemplateId) -> Result<Option<TemplateRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, name, slug, description, docker_image, startup_command, env_json, ports_json,
                memory_mb, cpu_shares, volume_path, source, body_json, created_at
         FROM templates WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(TemplateRecord::try_from).transpose()
}

pub async fn find_by_slug(
    pool: &MySqlPool,
    slug: &str,
) -> Result<Option<TemplateRecord>, sqlx::Error> {
    let row = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, name, slug, description, docker_image, startup_command, env_json, ports_json,
                memory_mb, cpu_shares, volume_path, source, body_json, created_at
         FROM templates WHERE slug = ?",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    row.map(TemplateRecord::try_from).transpose()
}

#[derive(sqlx::FromRow)]
struct TemplateRow {
    id: String,
    name: String,
    slug: String,
    description: String,
    docker_image: String,
    startup_command: Option<String>,
    env_json: serde_json::Value,
    ports_json: serde_json::Value,
    memory_mb: i32,
    cpu_shares: i32,
    volume_path: String,
    source: String,
    body_json: serde_json::Value,
    created_at: chrono::NaiveDateTime,
}

impl TryFrom<TemplateRow> for TemplateRecord {
    type Error = sqlx::Error;

    fn try_from(row: TemplateRow) -> Result<Self, Self::Error> {
        let ports: Vec<PortMapping> =
            serde_json::from_value(row.ports_json.clone()).unwrap_or_default();
        Ok(Self {
            summary: TemplateSummary {
                id: parse_id(&row.id, "templates.id")?,
                name: row.name,
                slug: row.slug,
                description: row.description,
                docker_image: row.docker_image,
                startup_command: row.startup_command.clone(),
                memory_mb: row.memory_mb,
                cpu_shares: row.cpu_shares,
                volume_path: row.volume_path.clone(),
                source: TemplateSource::parse(&row.source),
                ports,
                created_at: from_naive(row.created_at),
            },
            env_json: row.env_json.to_string(),
            body_json: row.body_json.to_string(),
            startup_command: row.startup_command,
            volume_path: row.volume_path,
            cpu_shares: row.cpu_shares,
        })
    }
}
