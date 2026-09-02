use axum::extract::State;
use axum::Json;
use fps_domain::{Permission, TemplateSummary};
use fps_templates::{import_egg, NativePort, NativeTemplate, NATIVE_TEMPLATE_KIND};
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::db::{audit, templates};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub docker_image: String,
    pub environment: Option<BTreeMap<String, String>>,
    pub ports: Option<Vec<NativePort>>,
    pub memory_mb: Option<u32>,
    pub startup: Option<String>,
    pub game: Option<String>,
}

#[utoipa::path(get, path = "/v1/templates", tag = "templates", responses((status = 200, body = [TemplateSummary])))]
pub async fn list_templates(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<TemplateSummary>>, ApiError> {
    auth.require(Permission::TemplatesRead)?;
    templates::ensure_catalogue(&state.pool).await?;
    let rows = templates::list(&state.pool).await?;
    Ok(Json(rows.into_iter().map(|r| r.summary).collect()))
}

#[utoipa::path(
    post,
    path = "/v1/templates",
    tag = "templates",
    request_body = serde_json::Value,
    responses((status = 200, body = TemplateSummary))
)]
pub async fn create_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateTemplateRequest>,
) -> Result<Json<TemplateSummary>, ApiError> {
    auth.require(Permission::TemplatesWrite)?;
    let native = NativeTemplate {
        kind: NATIVE_TEMPLATE_KIND.into(),
        schema_version: 1,
        name: body.name,
        slug: body.slug,
        game: body.game.unwrap_or_default(),
        description: body.description,
        docker_image: body.docker_image,
        startup: body.startup,
        environment: body.environment.unwrap_or_default(),
        ports: body.ports.unwrap_or_default(),
        memory_mb: body.memory_mb.unwrap_or(64),
        cpu_shares: 1024,
        volume_path: "/data".into(),
    }
    .with_defaults();
    native
        .validate()
        .map_err(|e| ApiError(fps_domain::PlatformError::validation(e)))?;
    if templates::find_by_slug(&state.pool, &native.slug)
        .await?
        .is_some()
    {
        return Err(ApiError(fps_domain::PlatformError::new(
            fps_domain::ErrorCode::Conflict,
            "A template with that slug already exists.",
        )));
    }
    let id = templates::insert_native(&state.pool, &native).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        None,
        "templates.created",
        "template",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "slug": native.slug }),
    )
    .await?;
    let rec = templates::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::internal()))?;
    Ok(Json(rec.summary))
}

#[utoipa::path(
    post,
    path = "/v1/templates/import-egg",
    tag = "templates",
    request_body = serde_json::Value,
    responses((status = 200, body = TemplateSummary))
)]
pub async fn import_egg_template(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<TemplateSummary>, ApiError> {
    auth.require(Permission::TemplatesWrite)?;
    let mut native =
        import_egg(&body).map_err(|e| ApiError(fps_domain::PlatformError::validation(e)))?;
    if templates::find_by_slug(&state.pool, &native.slug)
        .await?
        .is_some()
    {
        native.slug = format!("{}-{}", native.slug, &uuid::Uuid::now_v7().to_string()[..8]);
    }
    let id = templates::insert_imported(&state.pool, &native).await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        None,
        "templates.imported_egg",
        "template",
        Some(&id.to_string()),
        None,
        None,
        serde_json::json!({ "slug": native.slug }),
    )
    .await?;
    let rec = templates::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::internal()))?;
    Ok(Json(rec.summary))
}
