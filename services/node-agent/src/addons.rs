//! Download and extract curated game addons into a server volume.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use fps_protocol::{JobInstruction, JobResult};
use fps_templates::{asset_rank, glob_match, AddonSource, AddonSpec, ArchiveKind, FilePatch};
use serde::Deserialize;
use serde_json::Value;

use crate::docker;
use crate::jobs::{failed, ok, safe_rel_path};

const MAX_DOWNLOAD_BYTES: usize = 256 * 1024 * 1024;
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Deserialize)]
struct InstallPayload {
    container_name: String,
    spec: AddonSpec,
    #[serde(default)]
    restart: bool,
}

#[derive(Debug, Deserialize)]
struct UninstallPayload {
    container_name: String,
    #[serde(default)]
    tracked_paths: Vec<String>,
    #[serde(default)]
    post_uninstall: Vec<FilePatch>,
    #[serde(default)]
    restart: bool,
}

pub async fn install(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: InstallPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid addon_install payload: {err}")),
    };
    let volume = docker::volume_host_dir(data_dir, &payload.container_name);
    if let Err(err) = fs::create_dir_all(&volume) {
        return failed(job, format!("volume dir: {err}"));
    }
    let url = match resolve_download_url(&payload.spec.source).await {
        Ok(url) => url,
        Err(err) => return failed(job, err),
    };
    let bytes = match download(&url).await {
        Ok(b) => b,
        Err(err) => return failed(job, err),
    };
    let archive = match payload.spec.archive {
        ArchiveKind::File => ArchiveKind::File,
        other => {
            let inferred = ArchiveKind::infer(&url);
            if matches!(inferred, ArchiveKind::File) {
                other
            } else {
                inferred
            }
        }
    };
    if let Err(err) = extract_into(
        &volume,
        &payload.spec.dest_path,
        payload.spec.strip_components,
        archive,
        &bytes,
        payload.spec.slug.as_str(),
    ) {
        return failed(job, err);
    }
    for patch in &payload.spec.post_install {
        if let Err(err) = apply_patch(&volume, patch) {
            return failed(job, err);
        }
    }
    if payload.restart {
        let _ = restart_named(&payload.container_name).await;
    }
    let mut result = ok(
        job,
        format!("installed {} ({})", payload.spec.name, payload.spec.slug),
    );
    result.container_name = Some(payload.container_name);
    result.tracked_paths = Some(payload.spec.tracked_paths);
    result
}

pub async fn uninstall(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: UninstallPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid addon_uninstall payload: {err}")),
    };
    let volume = docker::volume_host_dir(data_dir, &payload.container_name);
    for patch in &payload.post_uninstall {
        if let Err(err) = apply_patch(&volume, patch) {
            return failed(job, err);
        }
    }
    for rel in &payload.tracked_paths {
        match safe_rel_path(rel) {
            Ok(path) => {
                let target = volume.join(path);
                if target.is_dir() {
                    if let Err(err) = fs::remove_dir_all(&target) {
                        return failed(job, format!("remove {}: {err}", target.display()));
                    }
                } else if target.is_file() {
                    if let Err(err) = fs::remove_file(&target) {
                        return failed(job, format!("remove {}: {err}", target.display()));
                    }
                }
            }
            Err(err) => return failed(job, err),
        }
    }
    if payload.restart {
        let _ = restart_named(&payload.container_name).await;
    }
    let mut result = ok(job, "uninstalled addon");
    result.container_name = Some(payload.container_name);
    result.tracked_paths = Some(payload.tracked_paths);
    result
}

async fn restart_named(container_name: &str) -> Result<(), String> {
    let docker = docker::connect().map_err(|err| err.to_string())?;
    let _ = docker::stop_named(&docker, container_name).await;
    docker::start_named(&docker, container_name).await
}

pub async fn resolve_download_url(source: &AddonSource) -> Result<String, String> {
    match source {
        AddonSource::Url { url } => Ok(url.clone()),
        AddonSource::GithubArchive {
            owner,
            repo,
            branch,
        } => Ok(format!(
            "https://github.com/{owner}/{repo}/archive/refs/heads/{branch}.zip"
        )),
        AddonSource::GithubRelease {
            owner,
            repo,
            asset_glob,
        } => github_release_asset(owner, repo, asset_glob).await,
        AddonSource::IndexHref {
            index_url,
            asset_glob,
        } => index_href_asset(index_url, asset_glob).await,
    }
}

async fn github_release_asset(owner: &str, repo: &str, glob: &str) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let body = http_text(&url).await?;
    let json: Value = serde_json::from_str(&body).map_err(|err| format!("github json: {err}"))?;
    let assets = json
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("GitHub release for {owner}/{repo} has no assets"))?;
    let mut matched: Vec<(String, String)> = Vec::new();
    for asset in assets {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or("");
        if glob_match(glob, name) {
            if let Some(dl) = asset.get("browser_download_url").and_then(Value::as_str) {
                matched.push((name.to_string(), dl.to_string()));
            }
        }
    }
    matched.sort_by_key(|a| asset_rank(&a.0));
    matched
        .pop()
        .map(|(_, url)| url)
        .ok_or_else(|| format!("no GitHub asset matching {glob} in {owner}/{repo}"))
}

async fn index_href_asset(index_url: &str, glob: &str) -> Result<String, String> {
    let body = http_text(index_url).await?;
    let mut matched: Vec<String> = Vec::new();
    for href in hrefs(&body) {
        let name = href.rsplit('/').next().unwrap_or(&href);
        if glob_match(glob, name) {
            matched.push(join_url(index_url, &href));
        }
    }
    matched.sort_by_key(|a| asset_rank(a.rsplit('/').next().unwrap_or(a)));
    matched
        .pop()
        .ok_or_else(|| format!("no download matching {glob} at {index_url}"))
}

fn hrefs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html;
    let mut rest = lower;
    while let Some(idx) = rest.find("href=") {
        rest = &rest[idx + 5..];
        let quote = rest.chars().next();
        if quote != Some('"') && quote != Some('\'') {
            continue;
        }
        let q = quote.unwrap();
        rest = &rest[q.len_utf8()..];
        if let Some(end) = rest.find(q) {
            out.push(rest[..end].to_string());
            rest = &rest[end + q.len_utf8()..];
        } else {
            break;
        }
    }
    out
}

fn join_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if let Some(scheme_end) = base.find("://") {
        let after = &base[scheme_end + 3..];
        if href.starts_with('/') {
            let host = after.split('/').next().unwrap_or(after);
            return format!("{}://{host}{href}", &base[..scheme_end]);
        }
    }
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/{href}")
}

async fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(fps_branding::user_agent())
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|err| err.to_string())
}

async fn http_text(url: &str) -> Result<String, String> {
    let client = http_client().await?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("GET {url}: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    resp.text()
        .await
        .map_err(|err| format!("read {url}: {err}"))
}

async fn download(url: &str) -> Result<Vec<u8>, String> {
    let client = http_client().await?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("download {url}: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!("download {url}: HTTP {}", resp.status()));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_DOWNLOAD_BYTES {
            return Err(format!(
                "download is larger than {MAX_DOWNLOAD_BYTES} bytes"
            ));
        }
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|err| format!("download body: {err}"))?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err(format!("download exceeded {MAX_DOWNLOAD_BYTES} bytes"));
    }
    if bytes.is_empty() {
        return Err("download was empty".into());
    }
    Ok(bytes.to_vec())
}

fn extract_into(
    volume: &Path,
    dest_path: &str,
    strip: u32,
    archive: ArchiveKind,
    bytes: &[u8],
    slug: &str,
) -> Result<(), String> {
    let dest = if dest_path.trim().is_empty() {
        volume.to_path_buf()
    } else {
        let rel = safe_rel_path(dest_path)?;
        volume.join(rel)
    };
    match archive {
        ArchiveKind::File => {
            let file = if dest.extension().is_some() || dest_path.contains('.') {
                dest
            } else {
                dest.join(format!("{slug}.bin"))
            };
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
            }
            fs::write(&file, bytes).map_err(|err| format!("write {}: {err}", file.display()))?;
            Ok(())
        }
        ArchiveKind::Zip | ArchiveKind::TarGz => {
            fs::create_dir_all(&dest).map_err(|err| format!("mkdir {}: {err}", dest.display()))?;
            if matches!(archive, ArchiveKind::Zip) {
                extract_zip(bytes, &dest, strip)
            } else {
                extract_tar_gz(bytes, &dest, strip)
            }
        }
    }
}

fn strip_rel(path: &str, strip: u32) -> Option<String> {
    let trimmed = path.replace('\\', "/").trim_start_matches('/').to_string();
    if trimmed.is_empty() || trimmed.contains("..") {
        return None;
    }
    let mut parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if strip as usize >= parts.len() {
        return None;
    }
    parts.drain(0..strip as usize);
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

fn extract_zip(bytes: &[u8], dest: &Path, strip: u32) -> Result<(), String> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|err| format!("zip: {err}"))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|err| format!("zip entry: {err}"))?;
        let name = file.name().to_string();
        if name.ends_with('/') {
            if let Some(rel) = strip_rel(&name, strip) {
                fs::create_dir_all(dest.join(rel)).map_err(|err| err.to_string())?;
            }
            continue;
        }
        let Some(rel) = strip_rel(&name, strip) else {
            continue;
        };
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut out = fs::File::create(&target).map_err(|err| err.to_string())?;
        std::io::copy(&mut file, &mut out).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn extract_tar_gz(bytes: &[u8], dest: &Path, strip: u32) -> Result<(), String> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|err| format!("tar: {err}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|err| format!("tar entry: {err}"))?;
        let path = entry
            .path()
            .map_err(|err| format!("tar path: {err}"))?
            .to_string_lossy()
            .into_owned();
        let Some(rel) = strip_rel(&path, strip) else {
            continue;
        };
        let target = dest.join(&rel);
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&target).map_err(|err| err.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut out = fs::File::create(&target).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn apply_patch(volume: &Path, patch: &FilePatch) -> Result<(), String> {
    match patch {
        FilePatch::EnsureLine {
            path,
            after_contains,
            line,
        } => {
            let rel = safe_rel_path(path)?;
            let file = volume.join(rel);
            if !file.is_file() {
                return Ok(());
            }
            let text = fs::read_to_string(&file)
                .map_err(|err| format!("read {}: {err}", file.display()))?;
            if text.lines().any(|l| l.trim() == line.trim()) {
                return Ok(());
            }
            let updated = insert_after(&text, after_contains.as_deref(), line);
            fs::write(&file, updated).map_err(|err| format!("write {}: {err}", file.display()))?;
            Ok(())
        }
        FilePatch::RemoveLine { path, contains } => {
            let rel = safe_rel_path(path)?;
            let file = volume.join(rel);
            if !file.is_file() {
                return Ok(());
            }
            let text = fs::read_to_string(&file)
                .map_err(|err| format!("read {}: {err}", file.display()))?;
            let needle = contains.trim();
            let updated: String = text
                .lines()
                .filter(|l| !l.contains(needle))
                .flat_map(|l| [l, "\n"])
                .collect();
            fs::write(&file, updated).map_err(|err| format!("write {}: {err}", file.display()))?;
            Ok(())
        }
    }
}

fn insert_after(text: &str, after_contains: Option<&str>, line: &str) -> String {
    let mut out = String::new();
    let mut inserted = false;
    for existing in text.lines() {
        out.push_str(existing);
        out.push('\n');
        if !inserted {
            if let Some(needle) = after_contains {
                if existing.contains(needle) {
                    out.push_str(line);
                    if !line.ends_with('\n') {
                        out.push('\n');
                    }
                    inserted = true;
                }
            }
        }
    }
    if !inserted {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        if !line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use zip::write::SimpleFileOptions;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fps-addon-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in files {
            writer.start_file(*name, opts).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn zip_extract_strips_and_rejects_parent() {
        let dir = temp_dir();
        let bytes = zip_bytes(&[
            ("outer/addons/hello.txt", b"hi"),
            ("outer/../escape.txt", b"nope"),
        ]);
        extract_zip(&bytes, &dir, 1).unwrap();
        assert_eq!(fs::read(dir.join("addons/hello.txt")).unwrap(), b"hi");
        assert!(!dir.join("escape.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_and_remove_gameinfo_line() {
        let dir = temp_dir();
        let gi = dir.join("gameinfo.gi");
        fs::write(
            &gi,
            "SearchPaths\n{\n\t\t\tGame_LowViolence\tcsgo_lv\n\t\t\tGame\tcsgo\n}\n",
        )
        .unwrap();
        apply_patch(
            &dir,
            &FilePatch::EnsureLine {
                path: "gameinfo.gi".into(),
                after_contains: Some("Game_LowViolence".into()),
                line: "\t\t\tGame\tcsgo/addons/metamod".into(),
            },
        )
        .unwrap();
        let text = fs::read_to_string(&gi).unwrap();
        assert!(text.contains("csgo/addons/metamod"));
        apply_patch(
            &dir,
            &FilePatch::EnsureLine {
                path: "gameinfo.gi".into(),
                after_contains: Some("Game_LowViolence".into()),
                line: "\t\t\tGame\tcsgo/addons/metamod".into(),
            },
        )
        .unwrap();
        assert_eq!(
            text.matches("csgo/addons/metamod").count(),
            fs::read_to_string(&gi)
                .unwrap()
                .matches("csgo/addons/metamod")
                .count()
        );
        apply_patch(
            &dir,
            &FilePatch::RemoveLine {
                path: "gameinfo.gi".into(),
                contains: "csgo/addons/metamod".into(),
            },
        )
        .unwrap();
        assert!(!fs::read_to_string(&gi).unwrap().contains("metamod"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn href_parser_picks_git_builds() {
        let html = r#"
            <a href="mmsource-latest-linux.tar.gz">latest</a>
            <a href="mmsource-2.0.0-git999-linux.tar.gz">old</a>
            <a href="mmsource-2.0.0-git1410-linux.tar.gz">new</a>
        "#;
        let links = hrefs(html);
        let mut matched: Vec<String> = links
            .into_iter()
            .filter(|h| glob_match("mmsource-2.0.0-git*-linux.tar.gz", h))
            .collect();
        matched.sort_by_key(|a| asset_rank(a));
        assert_eq!(
            matched.pop().unwrap(),
            "mmsource-2.0.0-git1410-linux.tar.gz"
        );
    }

    #[test]
    fn join_relative_and_absolute() {
        assert_eq!(
            join_url("https://example.test/drop/2.0/", "file.tar.gz"),
            "https://example.test/drop/2.0/file.tar.gz"
        );
        assert_eq!(
            join_url("https://example.test/drop/2.0/", "/abs/file.tar.gz"),
            "https://example.test/abs/file.tar.gz"
        );
    }

    #[tokio::test]
    async fn install_file_from_local_http() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/LuckPerms.jar"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(b"jar-bytes"))
            .mount(&server)
            .await;
        let data = temp_dir();
        let job: JobInstruction = serde_json::from_value(serde_json::json!({
            "id": "01234567-89ab-7def-8123-456789abcdef",
            "kind": "addon_install",
            "payload": {
                "server_id": "01234567-89ab-7def-8123-456789abcdef",
                "container_name": "fps-paper",
                "addon_slug": "mc-luckperms",
                "restart": false,
                "spec": {
                    "slug": "mc-luckperms",
                    "name": "LuckPerms",
                    "description": "test",
                    "category": "plugin",
                    "games": ["minecraft"],
                    "template_slugs": ["minecraft-paper"],
                    "version_label": "latest",
                    "source": { "type": "url", "url": format!("{}/LuckPerms.jar", server.uri()) },
                    "archive": "file",
                    "dest_path": "plugins/LuckPerms.jar",
                    "strip_components": 0,
                    "tracked_paths": ["plugins/LuckPerms.jar"],
                    "depends_on": [],
                    "restart_required": true,
                    "notes": "",
                    "homepage": null,
                    "post_install": []
                }
            }
        }))
        .unwrap();
        let result = install(&data, &job).await;
        assert!(result.success, "{}", result.message);
        let written = docker::volume_host_dir(&data, "fps-paper").join("plugins/LuckPerms.jar");
        assert_eq!(fs::read(&written).unwrap(), b"jar-bytes");
        let uninstall_job: JobInstruction = serde_json::from_value(serde_json::json!({
            "id": "01234567-89ab-7def-8123-456789abcdef",
            "kind": "addon_uninstall",
            "payload": {
                "container_name": "fps-paper",
                "tracked_paths": ["plugins/LuckPerms.jar"],
                "post_uninstall": [],
                "restart": false
            }
        }))
        .unwrap();
        let removed = uninstall(&data, &uninstall_job).await;
        assert!(removed.success, "{}", removed.message);
        assert!(!written.exists());
        let _ = fs::remove_dir_all(&data);
    }
}
