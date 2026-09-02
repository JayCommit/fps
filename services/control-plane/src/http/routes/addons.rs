use axum::extract::{Path, Query, State};
use axum::Json;
use fps_domain::{AddonInstallStatus, ErrorCode, JobKind, Permission, ServerId, ServerStatus};
use fps_templates::{addons_for_template, find_addon, AddonSpec};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{addons, audit, jobs, notifications, servers, templates};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::http::routes::servers::JobView;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CatalogueQuery {
    pub game: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AddonCatalogueItem {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub games: Vec<String>,
    pub template_slugs: Vec<String>,
    pub version_label: String,
    pub depends_on: Vec<String>,
    pub restart_required: bool,
    pub notes: String,
    pub homepage: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerAddonView {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub version_label: String,
    pub depends_on: Vec<String>,
    pub restart_required: bool,
    pub notes: String,
    pub homepage: Option<String>,
    /// `available`, `queued`, `installed`, `uninstalling`, or `failed`.
    pub status: String,
    pub error: Option<String>,
    pub installed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<&AddonSpec> for AddonCatalogueItem {
    fn from(spec: &AddonSpec) -> Self {
        Self {
            slug: spec.slug.clone(),
            name: spec.name.clone(),
            description: spec.description.clone(),
            category: spec.category.clone(),
            games: spec.games.clone(),
            template_slugs: spec.template_slugs.clone(),
            version_label: spec.version_label.clone(),
            depends_on: spec.depends_on.clone(),
            restart_required: spec.restart_required,
            notes: spec.notes.clone(),
            homepage: spec.homepage.clone(),
        }
    }
}

fn offer(spec: &AddonSpec, row: Option<&addons::AddonRecord>) -> ServerAddonView {
    let (status, error, installed_at) = match row {
        Some(rec) => (
            rec.summary.status.as_str().to_string(),
            rec.summary.error.clone(),
            rec.summary.installed_at,
        ),
        None => ("available".into(), None, None),
    };
    ServerAddonView {
        slug: spec.slug.clone(),
        name: spec.name.clone(),
        description: spec.description.clone(),
        category: spec.category.clone(),
        version_label: spec.version_label.clone(),
        depends_on: spec.depends_on.clone(),
        restart_required: spec.restart_required,
        notes: spec.notes.clone(),
        homepage: spec.homepage.clone(),
        status,
        error,
        installed_at,
    }
}

#[utoipa::path(
    get,
    path = "/v1/addons",
    tag = "servers",
    responses((status = 200, body = [AddonCatalogueItem]))
)]
pub async fn list_catalogue(
    auth: AuthUser,
    Query(q): Query<CatalogueQuery>,
) -> Result<Json<Vec<AddonCatalogueItem>>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let mut items: Vec<AddonCatalogueItem> = fps_templates::seeded_addons()
        .iter()
        .map(AddonCatalogueItem::from)
        .collect();
    if let Some(game) = q.game.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
        items.retain(|a| a.games.iter().any(|g| g == game));
    }
    Ok(Json(items))
}

#[utoipa::path(
    get,
    path = "/v1/servers/{id}/addons",
    tag = "servers",
    responses((status = 200, body = [ServerAddonView]))
)]
pub async fn list_server_addons(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<ServerAddonView>>, ApiError> {
    auth.require(Permission::ServersRead)?;
    let rec = load_server(&state, &id).await?;
    let template = templates::get(&state.pool, rec.summary.template_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("template")))?;
    let installed = addons::list_for_server(&state.pool, rec.summary.id).await?;
    let catalogue = addons_for_template(&template.summary.slug, &template.summary.game);
    let views = catalogue
        .iter()
        .map(|spec| {
            let row = installed.iter().find(|r| r.summary.addon_slug == spec.slug);
            offer(spec, row)
        })
        .collect();
    Ok(Json(views))
}

#[utoipa::path(
    post,
    path = "/v1/servers/{id}/addons/{slug}/install",
    tag = "servers",
    responses((status = 200, body = JobView))
)]
pub async fn install_addon(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, slug)): Path<(String, String)>,
) -> Result<Json<JobView>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let rec = load_server(&state, &id).await?;
    if matches!(rec.summary.status, ServerStatus::Deleting) {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "Cannot install addons while the server is being deleted.",
        )));
    }
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    let template = templates::get(&state.pool, rec.summary.template_id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("template")))?;
    let requested =
        find_addon(&slug).ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("addon")))?;
    if !requested.matches_template(&template.summary.slug, &template.summary.game) {
        return Err(ApiError(fps_domain::PlatformError::validation(
            "This addon is not available for this server's game.",
        )));
    }
    let existing = addons::list_for_server(&state.pool, rec.summary.id).await?;
    if existing.iter().any(|row| {
        row.summary.addon_slug == slug
            && matches!(
                row.summary.status,
                AddonInstallStatus::Queued | AddonInstallStatus::Uninstalling
            )
    }) {
        return Err(ApiError(fps_domain::PlatformError::new(
            ErrorCode::Conflict,
            "An install or uninstall for this addon is already in progress.",
        )));
    }
    let plan = install_plan(&requested, &existing)?;
    let mut last_job: Option<JobView> = None;
    for spec in plan {
        let job_id = jobs::enqueue(
            &state.pool,
            node_id,
            Some(rec.summary.id),
            JobKind::AddonInstall,
            serde_json::json!({
                "server_id": rec.summary.id,
                "container_name": rec.summary.container_name,
                "addon_slug": spec.slug,
                "spec": spec,
                "restart": spec.restart_required,
            }),
        )
        .await?;
        addons::upsert_queued(
            &state.pool,
            rec.summary.id,
            &spec,
            job_id,
            AddonInstallStatus::Queued,
        )
        .await?;
        last_job = Some(JobView {
            id: job_id,
            kind: JobKind::AddonInstall.as_str().into(),
            status: "queued".into(),
            result: None,
            created_at: chrono::Utc::now(),
        });
    }
    let Some(job) = last_job else {
        return Err(ApiError(fps_domain::PlatformError::internal()));
    };
    audit::record(
        &state.pool,
        Some(auth.user.id),
        rec.summary.node_id,
        "servers.addon_install",
        "server",
        Some(&rec.summary.id.to_string()),
        None,
        None,
        serde_json::json!({ "addon": slug }),
    )
    .await?;
    notifications::insert(
        &state.pool,
        "addon",
        "Addon install queued",
        &format!(
            "{} will be installed on {}.",
            requested.name, rec.summary.name
        ),
    )
    .await?;
    Ok(Json(job))
}

#[utoipa::path(
    post,
    path = "/v1/servers/{id}/addons/{slug}/uninstall",
    tag = "servers",
    responses((status = 200, body = JobView))
)]
pub async fn uninstall_addon(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, slug)): Path<(String, String)>,
) -> Result<Json<JobView>, ApiError> {
    auth.require(Permission::ServersWrite)?;
    let rec = load_server(&state, &id).await?;
    let node_id = rec
        .summary
        .node_id
        .ok_or_else(|| ApiError(fps_domain::PlatformError::validation("Server has no node.")))?;
    let existing = addons::list_for_server(&state.pool, rec.summary.id).await?;
    let row = existing
        .iter()
        .find(|r| r.summary.addon_slug == slug)
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("addon install")))?;
    if matches!(
        row.summary.status,
        AddonInstallStatus::Queued | AddonInstallStatus::Uninstalling
    ) {
        return Err(ApiError(fps_domain::PlatformError::new(
            ErrorCode::Conflict,
            "An install or uninstall for this addon is already in progress.",
        )));
    }
    let dependents: Vec<String> = existing
        .iter()
        .filter(|other| {
            other.summary.addon_slug != slug
                && matches!(
                    other.summary.status,
                    AddonInstallStatus::Installed
                        | AddonInstallStatus::Queued
                        | AddonInstallStatus::Uninstalling
                )
        })
        .filter_map(|other| {
            let spec = addons::parse_spec(&other.spec_json)?;
            spec.depends_on
                .iter()
                .any(|d| d == &slug)
                .then_some(other.summary.addon_name.clone())
        })
        .collect();
    if !dependents.is_empty() {
        return Err(ApiError(fps_domain::PlatformError::new(
            ErrorCode::Conflict,
            format!(
                "Uninstall {} first: {}.",
                if dependents.len() == 1 {
                    "this dependent addon"
                } else {
                    "these dependent addons"
                },
                dependents.join(", ")
            ),
        )));
    }
    let spec = addons::parse_spec(&row.spec_json)
        .or_else(|| find_addon(&slug))
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("addon")))?;
    let tracked = if row.summary.tracked_paths.is_empty() {
        spec.tracked_paths.clone()
    } else {
        row.summary.tracked_paths.clone()
    };
    let job_id = jobs::enqueue(
        &state.pool,
        node_id,
        Some(rec.summary.id),
        JobKind::AddonUninstall,
        serde_json::json!({
            "server_id": rec.summary.id,
            "container_name": rec.summary.container_name,
            "addon_id": row.summary.id,
            "addon_slug": slug,
            "tracked_paths": tracked,
            "post_uninstall": spec.uninstall_patches(),
            "restart": spec.restart_required,
        }),
    )
    .await?;
    addons::upsert_queued(
        &state.pool,
        rec.summary.id,
        &spec,
        job_id,
        AddonInstallStatus::Uninstalling,
    )
    .await?;
    audit::record(
        &state.pool,
        Some(auth.user.id),
        rec.summary.node_id,
        "servers.addon_uninstall",
        "server",
        Some(&rec.summary.id.to_string()),
        None,
        None,
        serde_json::json!({ "addon": slug }),
    )
    .await?;
    Ok(Json(JobView {
        id: job_id,
        kind: JobKind::AddonUninstall.as_str().into(),
        status: "queued".into(),
        result: None,
        created_at: chrono::Utc::now(),
    }))
}

fn install_plan(
    requested: &AddonSpec,
    existing: &[addons::AddonRecord],
) -> Result<Vec<AddonSpec>, ApiError> {
    let mut plan = Vec::new();
    let mut visiting = Vec::new();
    collect_plan(requested, existing, &mut plan, &mut visiting)?;
    Ok(plan)
}

fn collect_plan(
    spec: &AddonSpec,
    existing: &[addons::AddonRecord],
    plan: &mut Vec<AddonSpec>,
    visiting: &mut Vec<String>,
) -> Result<(), ApiError> {
    if plan.iter().any(|s| s.slug == spec.slug) {
        return Ok(());
    }
    if visiting.iter().any(|s| s == &spec.slug) {
        return Err(ApiError(fps_domain::PlatformError::internal()));
    }
    visiting.push(spec.slug.clone());
    for dep_slug in &spec.depends_on {
        let dep = find_addon(dep_slug).ok_or_else(|| {
            ApiError(fps_domain::PlatformError::validation(format!(
                "Unknown dependency {dep_slug}."
            )))
        })?;
        let installed = existing.iter().any(|row| {
            row.summary.addon_slug == *dep_slug
                && row.summary.status == AddonInstallStatus::Installed
        });
        let in_flight = existing.iter().any(|row| {
            row.summary.addon_slug == *dep_slug
                && matches!(
                    row.summary.status,
                    AddonInstallStatus::Queued | AddonInstallStatus::Uninstalling
                )
        });
        if installed || in_flight {
            continue;
        }
        collect_plan(&dep, existing, plan, visiting)?;
    }
    visiting.pop();
    plan.push(spec.clone());
    Ok(())
}

async fn load_server(state: &AppState, id: &str) -> Result<servers::ServerRecord, ApiError> {
    let id: ServerId = id
        .parse()
        .map_err(|_| ApiError(fps_domain::PlatformError::validation("invalid server id")))?;
    servers::get(&state.pool, id)
        .await?
        .ok_or_else(|| ApiError(fps_domain::PlatformError::not_found("server")))
}
