use axum::extract::State;
use axum::Json;
use fps_branding::{GITHUB_OWNER, GITHUB_REPOSITORY, VERSION};
use fps_domain::Permission;
use fps_updater::{github_releases_url, select_release, PublishedRelease};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{audit, settings};
use crate::http::error::ApiError;
use crate::http::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct PlatformSettingsView {
    pub product: String,
    pub version: String,
    pub public_url: String,
    pub allow_insecure_http: bool,
    pub heartbeat_timeout_secs: i64,
    pub cors_origins: Vec<String>,
    pub operator_notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchSettingsRequest {
    pub operator_notes: Option<String>,
}

#[utoipa::path(get, path = "/v1/settings", tag = "ops", responses((status = 200, body = PlatformSettingsView)))]
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<PlatformSettingsView>, ApiError> {
    auth.require(Permission::PlatformSettingsRead)?;
    let notes = settings::get_json(&state.pool, "operator_notes")
        .await?
        .and_then(|v| v.as_str().map(str::to_string));
    Ok(Json(PlatformSettingsView {
        product: fps_branding::DISPLAY_NAME.into(),
        version: VERSION.into(),
        public_url: state.config.public_url.clone(),
        allow_insecure_http: state.config.allow_insecure_http,
        heartbeat_timeout_secs: state.config.heartbeat_timeout_secs,
        cors_origins: state.config.cors_origins.clone(),
        operator_notes: notes,
    }))
}

#[utoipa::path(patch, path = "/v1/settings", tag = "ops", request_body = PatchSettingsRequest, responses((status = 200, body = PlatformSettingsView)))]
pub async fn patch_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PatchSettingsRequest>,
) -> Result<Json<PlatformSettingsView>, ApiError> {
    auth.require(Permission::PlatformSettingsWrite)?;
    if let Some(notes) = body.operator_notes {
        settings::put_json(
            &state.pool,
            "operator_notes",
            &serde_json::Value::String(notes),
        )
        .await?;
        audit::record(
            &state.pool,
            Some(auth.user.id),
            None,
            "settings.updated",
            "platform",
            None,
            None,
            None,
            serde_json::json!({}),
        )
        .await?;
    }
    get_settings(State(state), auth).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateCheck {
    pub current_version: String,
    pub channel: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub releases_url: String,
    pub message: String,
}

#[utoipa::path(get, path = "/v1/updates/check", tag = "ops", responses((status = 200, body = UpdateCheck)))]
pub async fn check_updates(
    State(_state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<UpdateCheck>, ApiError> {
    auth.require(Permission::DiagnosticsRead)?;
    let channel = fps_branding::implied_channel();
    let current = fps_branding::version();
    let url = github_releases_url(GITHUB_OWNER, GITHUB_REPOSITORY, channel);
    let client = reqwest::Client::builder()
        .user_agent(fps_branding::user_agent())
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|_| ApiError(fps_domain::PlatformError::internal()))?;
    let response = client.get(&url).send().await;
    let Ok(response) = response else {
        return Ok(Json(UpdateCheck {
            current_version: VERSION.into(),
            channel: channel.as_str().into(),
            latest: None,
            update_available: false,
            releases_url: format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPOSITORY}/releases"),
            message: "Could not reach GitHub Releases. This alpha lists /releases, never /releases/latest.".into(),
        }));
    };
    if !response.status().is_success() {
        return Ok(Json(UpdateCheck {
            current_version: VERSION.into(),
            channel: channel.as_str().into(),
            latest: None,
            update_available: false,
            releases_url: format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPOSITORY}/releases"),
            message: format!(
                "GitHub returned {}. No matching release, or GitHub is rate-limiting this check.",
                response.status()
            ),
        }));
    }
    let payload: Vec<GhRelease> = response.json().await.unwrap_or_default();
    let releases: Vec<PublishedRelease> = payload
        .into_iter()
        .filter_map(|rel| {
            let version = match tag_version(&rel.tag_name) {
                Some(v) => v,
                None => return None,
            };
            Some(PublishedRelease {
                tag: rel.tag_name,
                prerelease: rel.prerelease || !version.pre.is_empty(),
                draft: rel.draft,
                version,
            })
        })
        .collect();
    match select_release(channel, &current, &releases) {
        Ok(chosen) => Ok(Json(UpdateCheck {
            current_version: VERSION.into(),
            channel: channel.as_str().into(),
            latest: Some(chosen.version.to_string()),
            update_available: true,
            releases_url: format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPOSITORY}/releases"),
            message: format!("A newer release is available: {}.", chosen.tag),
        })),
        Err(_) => Ok(Json(UpdateCheck {
            current_version: VERSION.into(),
            channel: channel.as_str().into(),
            latest: None,
            update_available: false,
            releases_url: format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPOSITORY}/releases"),
            message: "You are on the newest eligible release for this channel.".into(),
        })),
    }
}

fn tag_version(tag: &str) -> Option<semver::Version> {
    semver::Version::parse(tag.trim_start_matches('v')).ok()
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}
