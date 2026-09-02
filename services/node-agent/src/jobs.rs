//! Desired-state jobs executed on the node: install, start, stop, backup, restore, files.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fps_domain::{JobKind, ServerId};
use fps_protocol::{JobInstruction, JobResult, LogChunk};
use serde::Deserialize;
use tracing::{info, warn};

use crate::docker;
use crate::AgentRuntime;

pub async fn execute(data_dir: &Path, job: &JobInstruction) -> JobResult {
    execute_with_logs(data_dir, job, None).await
}

pub async fn execute_with_logs(
    data_dir: &Path,
    job: &JobInstruction,
    logs: Option<&AgentRuntime>,
) -> JobResult {
    let result = match job.kind {
        JobKind::Install => install(data_dir, job, logs).await,
        JobKind::Start => start(job).await,
        JobKind::Stop => stop(job).await,
        JobKind::Backup => backup(data_dir, job).await,
        JobKind::Restore => restore(data_dir, job).await,
        JobKind::FilesList => files_list(data_dir, job).await,
        JobKind::FilesRead => files_read(data_dir, job).await,
        JobKind::FilesWrite => files_write(data_dir, job).await,
        JobKind::Exec => exec_command(job).await,
        JobKind::AddonInstall => crate::addons::install(data_dir, job).await,
        JobKind::AddonUninstall => crate::addons::uninstall(data_dir, job).await,
        JobKind::Delete => delete(data_dir, job).await,
    };
    if result.success {
        info!(job_id = %job.id, kind = job.kind.as_str(), "job succeeded");
    } else {
        warn!(job_id = %job.id, kind = job.kind.as_str(), message = %result.message, "job failed");
    }
    result
}

pub(crate) fn failed(job: &JobInstruction, message: impl Into<String>) -> JobResult {
    JobResult {
        id: job.id,
        success: false,
        message: docker::redact(&message.into()),
        container_id: None,
        container_name: None,
        log_excerpt: None,
        backup_path: None,
        backup_bytes: None,
        files: None,
        file_content: None,
        tracked_paths: None,
        error_code: None,
    }
}

pub(crate) fn ok(job: &JobInstruction, message: impl Into<String>) -> JobResult {
    JobResult {
        id: job.id,
        success: true,
        message: message.into(),
        container_id: None,
        container_name: None,
        log_excerpt: None,
        backup_path: None,
        backup_bytes: None,
        files: None,
        file_content: None,
        tracked_paths: None,
        error_code: None,
    }
}

#[derive(Debug, Deserialize)]
struct InstallPayload {
    server_id: String,
    #[serde(default)]
    name: Option<String>,
    image: String,
    #[serde(default)]
    env: std::collections::HashMap<String, String>,
    #[serde(default)]
    cmd: Option<Vec<String>>,
    #[serde(default)]
    ports: Vec<PortSpec>,
    #[serde(default)]
    memory_mb: u64,
    #[serde(default = "default_volume_path")]
    volume_path: String,
    container_name: String,
    #[serde(default)]
    replace: bool,
}

fn default_volume_path() -> String {
    "/data".into()
}

#[derive(Debug, Deserialize)]
struct PortSpec {
    host: u16,
    container: u16,
    #[serde(default)]
    protocol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NamedContainerPayload {
    container_name: String,
}

#[derive(Debug, Deserialize)]
struct BackupPayload {
    container_name: String,
    backup_id: String,
    #[serde(default)]
    archive_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeletePayload {
    container_name: String,
    #[serde(default)]
    #[allow(dead_code)]
    server_id: String,
}

async fn emit_log(
    logs: Option<&AgentRuntime>,
    server_id: &str,
    stream: &str,
    text: impl Into<String>,
    max_chars: usize,
) {
    let Some(runtime) = logs else {
        return;
    };
    let Ok(server_id) = server_id.parse::<ServerId>() else {
        return;
    };
    let text = truncate_log(&text.into(), max_chars);
    if text.trim().is_empty() {
        return;
    }
    runtime.pending_logs.lock().await.push(LogChunk {
        server_id,
        stream: stream.into(),
        text,
    });
}

fn truncate_log(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((idx, _)) => format!("{}…", &text[..idx]),
        None => text.to_string(),
    }
}

async fn install(data_dir: &Path, job: &JobInstruction, logs: Option<&AgentRuntime>) -> JobResult {
    let payload: InstallPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid install payload: {err}")),
    };
    if payload.image.trim().is_empty() || payload.container_name.trim().is_empty() {
        return failed(job, "install requires image and container_name");
    }
    let server_id = payload.server_id.clone();
    let container_name = payload.container_name.clone();
    let image = payload.image.clone();
    let docker = match docker::connect() {
        Ok(d) => d,
        Err(err) => return failed(job, format!("docker unavailable: {err}")),
    };

    if payload.replace {
        emit_log(
            logs,
            &server_id,
            "install",
            format!("replacing container {container_name}"),
            240,
        )
        .await;
        let _ = docker::stop_named(&docker, &container_name).await;
        if let Err(err) = docker::remove_named(&docker, &container_name).await {
            return failed(job, format!("replace remove failed: {err}"));
        }
    }

    emit_log(
        logs,
        &server_id,
        "install",
        format!("pulling image {image}"),
        240,
    )
    .await;
    if let Err(err) = docker::pull_image_with_progress(&docker, &image, |line| {
        let server_id = server_id.clone();
        async move {
            emit_log(logs, &server_id, "install", line, 240).await;
        }
    })
    .await
    {
        return failed(job, format!("image pull failed: {err}"));
    }

    let host_dir = docker::volume_host_dir(data_dir, &container_name);
    if let Err(err) = tokio::fs::create_dir_all(&host_dir).await {
        return failed(job, format!("volume dir: {err}"));
    }
    let volume_path = if payload.volume_path.trim().is_empty() {
        default_volume_path()
    } else {
        payload.volume_path.clone()
    };
    let ports: Vec<docker::PortPublish> = payload
        .ports
        .into_iter()
        .map(|p| docker::PortPublish {
            host: p.host,
            container: p.container,
            protocol: protocol_or_tcp(p.protocol),
        })
        .collect();
    if !ports.is_empty() {
        let published = ports
            .iter()
            .map(|p| format!("{}->{}/{}", p.host, p.container, p.protocol))
            .collect::<Vec<_>>()
            .join(", ");
        emit_log(
            logs,
            &server_id,
            "install",
            format!("publishing ports {published}"),
            240,
        )
        .await;
    }
    emit_log(logs, &server_id, "install", "creating container", 240).await;
    let spec = docker::WorkloadCreate {
        container_name: container_name.clone(),
        image,
        env: payload.env.into_iter().collect(),
        cmd: payload.cmd,
        ports,
        memory_mb: payload.memory_mb,
        host_dir,
        volume_path,
        server_id: server_id.clone(),
    };
    emit_log(logs, &server_id, "install", "starting", 240).await;
    match docker::create_and_start_workload(&docker, &spec).await {
        Ok((container_id, started_name)) => {
            emit_log(
                logs,
                &server_id,
                "install",
                format!("started {started_name}"),
                240,
            )
            .await;
            if let Ok(text) = docker::tail_logs(&docker, &started_name, "20").await {
                emit_log(logs, &server_id, "stdout", text, 2048).await;
            }
            let mut result = ok(
                job,
                payload
                    .name
                    .as_deref()
                    .map(|n| format!("installed {n}"))
                    .unwrap_or_else(|| format!("installed {started_name}")),
            );
            result.container_id = Some(container_id);
            result.container_name = Some(started_name);
            result
        }
        Err(err) => {
            if docker::is_port_bind_conflict(&err) {
                let message = match docker::parse_conflict_host_port(&err) {
                    Some(port) => format!("Host port {port} is already in use on this node."),
                    None => "A host port is already in use on this node.".into(),
                };
                let mut result = failed(job, message);
                result.error_code = Some("port_conflict".into());
                result.container_name = Some(container_name);
                result
            } else {
                let mut result = failed(job, err);
                result.container_name = Some(container_name);
                result
            }
        }
    }
}

async fn delete(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: DeletePayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid delete payload: {err}")),
    };
    if payload.container_name.trim().is_empty() {
        return failed(job, "delete requires container_name");
    }
    if let Ok(docker) = docker::connect() {
        if docker.ping().await.is_ok() {
            let _ = docker::stop_named(&docker, &payload.container_name).await;
            if let Err(err) = docker::remove_named(&docker, &payload.container_name).await {
                return failed(job, err);
            }
        }
    }
    let host = docker::volume_host_dir(data_dir, &payload.container_name);
    match tokio::fs::remove_dir_all(&host).await {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return failed(job, format!("volume dir: {err}")),
    }
    let mut result = ok(job, format!("deleted {}", payload.container_name));
    result.container_name = Some(payload.container_name);
    result
}

async fn start(job: &JobInstruction) -> JobResult {
    let payload: NamedContainerPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid start payload: {err}")),
    };
    let docker = match docker::connect() {
        Ok(d) => d,
        Err(err) => return failed(job, format!("docker unavailable: {err}")),
    };
    match docker::inspect_named(&docker, &payload.container_name).await {
        Ok(inspect) if docker::container_is_running(&inspect) => {
            let mut result = ok(job, "already running");
            result.container_id = inspect.id;
            result.container_name = Some(payload.container_name);
            result
        }
        Ok(inspect) => match docker::start_named(&docker, &payload.container_name).await {
            Ok(()) => {
                let mut result = ok(job, "started");
                result.container_id = inspect.id;
                result.container_name = Some(payload.container_name);
                result
            }
            Err(err) => {
                let mut result = failed(job, err);
                result.container_name = Some(payload.container_name);
                result
            }
        },
        Err(err) => {
            let mut result = failed(job, format!("container missing: {err}"));
            result.container_name = Some(payload.container_name);
            result
        }
    }
}

async fn stop(job: &JobInstruction) -> JobResult {
    let payload: NamedContainerPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid stop payload: {err}")),
    };
    let docker = match docker::connect() {
        Ok(d) => d,
        Err(err) => return failed(job, format!("docker unavailable: {err}")),
    };
    match docker::inspect_named(&docker, &payload.container_name).await {
        Ok(inspect) if !docker::container_is_running(&inspect) => {
            let mut result = ok(job, "already stopped");
            result.container_id = inspect.id;
            result.container_name = Some(payload.container_name);
            result
        }
        Ok(inspect) => match docker::stop_named(&docker, &payload.container_name).await {
            Ok(()) => {
                let mut result = ok(job, "stopped");
                result.container_id = inspect.id;
                result.container_name = Some(payload.container_name);
                result
            }
            Err(err) => {
                let mut result = failed(job, err);
                result.container_name = Some(payload.container_name);
                result
            }
        },
        Err(err) => {
            let mut result = failed(job, format!("container missing: {err}"));
            result.container_name = Some(payload.container_name);
            result
        }
    }
}

async fn backup(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: BackupPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid backup payload: {err}")),
    };
    let Some(backup_id) = sanitize_id(&payload.backup_id) else {
        return failed(job, "invalid backup_id");
    };
    let dest = payload
        .archive_path
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| docker::backup_archive_path(data_dir, backup_id));
    if let Some(parent) = dest.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return failed(job, format!("backup dir: {err}"));
        }
    }

    let source = resolve_volume_dir(data_dir, &payload.container_name).await;
    let tar = match &source {
        Some(dir) => match archive::archive_directory(dir) {
            Ok(bytes) => bytes,
            Err(err) => return failed(job, format!("archive: {err}")),
        },
        None => archive::marker_archive(),
    };
    let gz = archive::gzip_wrap(&tar);
    if let Err(err) = fs::write(&dest, &gz) {
        return failed(job, format!("write archive: {err}"));
    }
    let mut result = ok(
        job,
        source
            .as_ref()
            .map(|_| "backup written")
            .unwrap_or("backup written (empty volume)"),
    );
    result.container_name = Some(payload.container_name);
    result.backup_path = Some(dest.display().to_string());
    result.backup_bytes = Some(gz.len() as u64);
    result
}

async fn restore(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: BackupPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid restore payload: {err}")),
    };
    let Some(backup_id) = sanitize_id(&payload.backup_id) else {
        return failed(job, "invalid backup_id");
    };
    let archive_path = payload
        .archive_path
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| docker::backup_archive_path(data_dir, backup_id));
    let gz = match fs::read(&archive_path) {
        Ok(bytes) => bytes,
        Err(err) => return failed(job, format!("read archive: {err}")),
    };
    let tar = match archive::gzip_unwrap(&gz) {
        Ok(bytes) => bytes,
        Err(err) => return failed(job, format!("decompress: {err}")),
    };

    let dest = match resolve_volume_dir(data_dir, &payload.container_name).await {
        Some(dir) => dir,
        None => {
            let host = docker::volume_host_dir(data_dir, &payload.container_name);
            if let Err(err) = fs::create_dir_all(&host) {
                return failed(job, format!("restore dir: {err}"));
            }
            host
        }
    };
    if let Err(err) = archive::extract_tar(&tar, &dest) {
        return failed(job, format!("extract: {err}"));
    }
    let mut result = ok(job, "restored");
    result.container_name = Some(payload.container_name);
    result.backup_path = Some(archive_path.display().to_string());
    result.backup_bytes = Some(gz.len() as u64);
    result
}

async fn files_list(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: NamedContainerPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid files_list payload: {err}")),
    };
    let mut files: Vec<FileEntry> = Vec::new();
    let mut listed = false;

    // Host volume is the source of truth (read/write jobs use the same path).
    // Minimal images such as http-echo often have no `ls` binary.
    let host = docker::volume_host_dir(data_dir, &payload.container_name);
    if host.is_dir() {
        match list_host_dir(&host) {
            Ok(entries) => {
                files = entries;
                listed = true;
            }
            Err(err) => return failed(job, format!("list files: {err}")),
        }
    }
    if !listed {
        if let Ok(docker) = docker::connect() {
            if let Ok(text) = docker::exec_ls(&docker, &payload.container_name, "/data").await {
                if !looks_like_exec_failure(&text) {
                    files = parse_ls_la(&text);
                    listed = true;
                }
            }
        }
    }

    if !listed {
        let mut result = failed(job, "container and host volume missing");
        result.container_name = Some(payload.container_name);
        result.files = Some(serde_json::json!([]));
        return result;
    }

    let mut result = ok(job, format!("{} entries", files.len()));
    result.container_name = Some(payload.container_name);
    result.files = Some(serde_json::to_value(&files).unwrap_or_else(|_| serde_json::json!([])));
    result
}

fn looks_like_exec_failure(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("executable file not found")
        || lower.contains("no such file or directory")
        || lower.contains("oci runtime exec failed")
}

#[derive(Debug, serde::Serialize, Deserialize)]
struct FileEntry {
    name: String,
    #[serde(default)]
    path: String,
    size: u64,
    #[serde(default)]
    is_dir: bool,
}

#[derive(Debug, Deserialize)]
struct FilePathPayload {
    container_name: String,
    path: String,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecPayload {
    container_name: String,
    command: String,
}

pub(crate) fn safe_rel_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Err("path is required".into());
    }
    let path = PathBuf::from(trimmed);
    if path.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::Prefix(_)
        )
    }) {
        return Err("path must stay inside the server volume".into());
    }
    Ok(path)
}

async fn files_read(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: FilePathPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid files_read payload: {err}")),
    };
    let rel = match safe_rel_path(&payload.path) {
        Ok(p) => p,
        Err(err) => return failed(job, err),
    };
    let host = docker::volume_host_dir(data_dir, &payload.container_name).join(&rel);
    match fs::read(&host) {
        Ok(bytes) => {
            if bytes.len() > 512 * 1024 {
                return failed(job, "file is larger than 512 KiB");
            }
            let mut result = ok(job, format!("read {}", rel.display()));
            result.container_name = Some(payload.container_name);
            result.file_content = Some(String::from_utf8_lossy(&bytes).into_owned());
            result
        }
        Err(err) => failed(job, format!("read {}: {err}", host.display())),
    }
}

async fn files_write(data_dir: &Path, job: &JobInstruction) -> JobResult {
    let payload: FilePathPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid files_write payload: {err}")),
    };
    let rel = match safe_rel_path(&payload.path) {
        Ok(p) => p,
        Err(err) => return failed(job, err),
    };
    let Some(content) = payload.content else {
        return failed(job, "content is required");
    };
    if content.len() > 512 * 1024 {
        return failed(job, "content is larger than 512 KiB");
    }
    let host = docker::volume_host_dir(data_dir, &payload.container_name).join(&rel);
    if let Some(parent) = host.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return failed(job, format!("mkdir: {err}"));
        }
    }
    match fs::write(&host, content.as_bytes()) {
        Ok(()) => {
            let mut result = ok(job, format!("wrote {}", rel.display()));
            result.container_name = Some(payload.container_name);
            result
        }
        Err(err) => failed(job, format!("write {}: {err}", host.display())),
    }
}

async fn exec_command(job: &JobInstruction) -> JobResult {
    let payload: ExecPayload = match serde_json::from_value(job.payload.clone()) {
        Ok(p) => p,
        Err(err) => return failed(job, format!("invalid exec payload: {err}")),
    };
    if payload.command.trim().is_empty() {
        return failed(job, "command is required");
    }
    let docker = match docker::connect() {
        Ok(d) => d,
        Err(err) => return failed(job, format!("docker unavailable: {err}")),
    };
    match docker::exec_shell(&docker, &payload.container_name, &payload.command).await {
        Ok(text) => {
            let mut result = ok(job, "exec complete");
            result.container_name = Some(payload.container_name);
            result.log_excerpt = Some(text);
            result
        }
        Err(err) => {
            let mut result = failed(job, err);
            result.container_name = Some(payload.container_name);
            result
        }
    }
}

fn list_host_dir(dir: &Path) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    let read = fs::read_dir(dir).map_err(|err| err.to_string())?;
    for item in read {
        let item = item.map_err(|err| err.to_string())?;
        let name = item.file_name().to_string_lossy().into_owned();
        if name == "." || name == ".." {
            continue;
        }
        let size = item.metadata().map(|m| m.len()).unwrap_or(0);
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(FileEntry {
            name: name.clone(),
            path: name,
            size,
            is_dir,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn parse_ls_la(text: &str) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("total ") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }
        let name = parts[8..].join(" ");
        if name == "." || name == ".." {
            continue;
        }
        let size = parts[4].parse().unwrap_or(0);
        let is_dir = parts[0].starts_with('d');
        entries.push(FileEntry {
            name: name.clone(),
            path: name,
            size,
            is_dir,
        });
    }
    entries
}

async fn resolve_volume_dir(data_dir: &Path, container_name: &str) -> Option<PathBuf> {
    if let Ok(docker) = docker::connect() {
        if let Ok(vol) = docker::inspect_named_volume(&docker, container_name).await {
            let mount = PathBuf::from(&vol.mountpoint);
            if mount.is_dir() {
                return Some(mount);
            }
        }
    }
    let host = docker::volume_host_dir(data_dir, container_name);
    if host.is_dir() {
        Some(host)
    } else {
        None
    }
}

fn protocol_or_tcp(protocol: Option<String>) -> String {
    match protocol {
        Some(p) if !p.trim().is_empty() => p,
        _ => "tcp".into(),
    }
}

fn sanitize_id(value: &str) -> Option<&str> {
    if value.is_empty() {
        return None;
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(value)
    } else {
        None
    }
}

mod archive {
    use super::*;

    pub fn marker_archive() -> Vec<u8> {
        match write_files(&[("MARKER", b"fps empty backup\n")]) {
            Ok(bytes) => bytes,
            Err(_) => vec![0; 1024],
        }
    }

    pub fn archive_directory(dir: &Path) -> Result<Vec<u8>, String> {
        let mut files = Vec::new();
        collect_files(dir, dir, &mut files)?;
        if files.is_empty() {
            return Ok(marker_archive());
        }
        let owned: Vec<(String, Vec<u8>)> = files;
        let refs: Vec<(&str, &[u8])> = owned
            .iter()
            .map(|(name, data)| (name.as_str(), data.as_slice()))
            .collect();
        write_files(&refs)
    }

    fn collect_files(
        root: &Path,
        current: &Path,
        out: &mut Vec<(String, Vec<u8>)>,
    ) -> Result<(), String> {
        let read = fs::read_dir(current).map_err(|err| err.to_string())?;
        for item in read {
            let item = item.map_err(|err| err.to_string())?;
            let path = item.path();
            let rel = path
                .strip_prefix(root)
                .map_err(|err| err.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if rel.is_empty() || rel.contains("..") {
                continue;
            }
            if path.is_dir() {
                collect_files(root, &path, out)?;
            } else if path.is_file() {
                let data = fs::read(&path).map_err(|err| err.to_string())?;
                out.push((rel, data));
            }
        }
        Ok(())
    }

    pub(crate) fn write_files(files: &[(&str, &[u8])]) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for (name, data) in files {
            if name.len() > 100 {
                return Err(format!("tar name too long: {name}"));
            }
            out.extend_from_slice(&ustar_header(name, data.len() as u64, b'0')?);
            out.extend_from_slice(data);
            pad_to_512(&mut out, data.len());
        }
        out.extend_from_slice(&[0u8; 1024]);
        Ok(out)
    }

    fn ustar_header(name: &str, size: u64, typeflag: u8) -> Result<[u8; 512], String> {
        let mut hdr = [0u8; 512];
        write_str(&mut hdr[0..100], name)?;
        write_octal(&mut hdr[100..108], 0o644);
        write_octal(&mut hdr[108..116], 0);
        write_octal(&mut hdr[116..124], 0);
        write_octal(&mut hdr[124..136], size);
        let mtime = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        write_octal(&mut hdr[136..148], mtime);
        hdr[156] = typeflag;
        hdr[257..263].copy_from_slice(b"ustar\0");
        hdr[263..265].copy_from_slice(b"00");
        hdr[148..156].fill(b' ');
        let sum: u32 = hdr.iter().map(|b| u32::from(*b)).sum();
        let chk = format!("{sum:06o}\0 ");
        hdr[148..156].copy_from_slice(chk.as_bytes());
        Ok(hdr)
    }

    fn write_str(slot: &mut [u8], value: &str) -> Result<(), String> {
        let bytes = value.as_bytes();
        if bytes.len() >= slot.len() {
            return Err("tar field overflow".into());
        }
        slot[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    fn write_octal(slot: &mut [u8], value: u64) {
        let width = slot.len().saturating_sub(1);
        let formatted = format!("{value:0width$o}");
        let bytes = formatted.as_bytes();
        let copy = bytes.len().min(width);
        slot[..copy].copy_from_slice(&bytes[..copy]);
        slot[slot.len() - 1] = 0;
    }

    fn pad_to_512(out: &mut Vec<u8>, data_len: usize) {
        let rem = data_len % 512;
        if rem != 0 {
            out.extend(std::iter::repeat_n(0u8, 512 - rem));
        }
    }

    pub fn gzip_wrap(tar: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff]);
        let mut offset = 0;
        while offset < tar.len() || tar.is_empty() {
            let remaining = tar.len() - offset;
            let chunk = remaining.min(65535);
            let last = offset + chunk >= tar.len();
            out.push(if last { 0x01 } else { 0x00 });
            let len = chunk as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(&tar[offset..offset + chunk]);
            offset += chunk;
            if tar.is_empty() {
                break;
            }
        }
        out.extend_from_slice(&crc32(tar).to_le_bytes());
        out.extend_from_slice(&(tar.len() as u32).to_le_bytes());
        out
    }

    pub fn gzip_unwrap(bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
            inflate_stored(bytes)
        } else if bytes.len() >= 512 {
            Ok(bytes.to_vec())
        } else {
            Err("unrecognized archive".into())
        }
    }

    fn inflate_stored(bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() < 18 {
            return Err("truncated gzip".into());
        }
        if bytes[2] != 8 {
            return Err("unsupported gzip method".into());
        }
        let flg = bytes[3];
        if flg != 0 {
            return Err("gzip flags not supported".into());
        }
        let mut i = 10;
        let mut out = Vec::new();
        loop {
            if i >= bytes.len() {
                return Err("truncated deflate".into());
            }
            let header = bytes[i];
            i += 1;
            let bfinal = header & 1;
            let btype = (header >> 1) & 0b11;
            if btype != 0 {
                return Err("compressed deflate blocks are not supported".into());
            }
            if i + 4 > bytes.len() {
                return Err("truncated stored block".into());
            }
            let len = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
            i += 2;
            let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
            i += 2;
            if nlen != !len {
                return Err("stored block length mismatch".into());
            }
            let len = usize::from(len);
            if i + len > bytes.len() {
                return Err("truncated stored data".into());
            }
            out.extend_from_slice(&bytes[i..i + len]);
            i += len;
            if bfinal == 1 {
                break;
            }
        }
        Ok(out)
    }

    pub fn extract_tar(tar: &[u8], dest: &Path) -> Result<(), String> {
        let mut cur = Cursor::new(tar);
        let mut hdr = [0u8; 512];
        loop {
            let n = cur.read(&mut hdr).map_err(|err| err.to_string())?;
            if n < 512 || hdr.iter().all(|b| *b == 0) {
                break;
            }
            let name = parse_c_str(&hdr[0..100]);
            if name.is_empty() || name.contains("..") || name.starts_with('/') {
                return Err("unsafe tar path".into());
            }
            let size = parse_octal(&hdr[124..136])?;
            let typeflag = hdr[156];
            let mut data = vec![0u8; size as usize];
            cur.read_exact(&mut data).map_err(|err| err.to_string())?;
            let pad = (512 - (size as usize % 512)) % 512;
            if pad > 0 {
                let mut skip = vec![0u8; pad];
                cur.read_exact(&mut skip).map_err(|err| err.to_string())?;
            }
            let target = dest.join(&name);
            if typeflag == b'5' {
                fs::create_dir_all(&target).map_err(|err| err.to_string())?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                fs::write(&target, data).map_err(|err| err.to_string())?;
            }
        }
        Ok(())
    }

    fn parse_c_str(bytes: &[u8]) -> String {
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    fn parse_octal(bytes: &[u8]) -> Result<u64, String> {
        let s = parse_c_str(bytes).replace(' ', "");
        if s.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(&s, 8).map_err(|err| err.to_string())
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in data {
            let idx = ((crc ^ u32::from(b)) & 0xff) as usize;
            crc = CRC32_TABLE[idx] ^ (crc >> 8);
        }
        !crc
    }

    const CRC32_TABLE: [u32; 256] = crc32_table();

    const fn crc32_table() -> [u32; 256] {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut j = 0;
            while j < 8 {
                if crc & 1 == 1 {
                    crc = 0xedb8_8320 ^ (crc >> 1);
                } else {
                    crc >>= 1;
                }
                j += 1;
            }
            table[i] = crc;
            i += 1;
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instruction(kind: &str, payload: serde_json::Value) -> JobInstruction {
        serde_json::from_value(json!({
            "id": "01234567-89ab-7def-8123-456789abcdef",
            "kind": kind,
            "payload": payload
        }))
        .expect("job instruction")
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pn-jobs-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[tokio::test]
    async fn invalid_payload_does_not_panic() {
        let job = instruction("install", json!({"nope": true}));
        let result = execute(Path::new("/tmp"), &job).await;
        assert!(!result.success);
        assert!(result.message.contains("invalid"));
    }

    #[tokio::test]
    async fn backup_without_volume_writes_marker_archive() {
        let dir = temp_dir();
        let job = instruction(
            "backup",
            json!({
                "server_id": "01234567-89ab-7def-8123-456789abcdef",
                "container_name": "pn-missing",
                "backup_id": "b1"
            }),
        );
        let result = execute(&dir, &job).await;
        assert!(result.success, "{}", result.message);
        let path = crate::docker::backup_archive_path(&dir, "b1");
        assert!(path.is_file());
        assert!(result.backup_bytes.unwrap_or(0) > 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn files_list_falls_back_to_host_dir() {
        let dir = temp_dir();
        let host = crate::docker::volume_host_dir(&dir, "pn-files");
        fs::create_dir_all(&host).unwrap();
        fs::write(host.join("hello.txt"), b"hello").unwrap();
        let job = instruction(
            "files_list",
            json!({
                "server_id": "01234567-89ab-7def-8123-456789abcdef",
                "container_name": "pn-files"
            }),
        );
        let result = execute(&dir, &job).await;
        assert!(result.success, "{}", result.message);
        let files = result.files.expect("files");
        let arr = files.as_array().expect("array");
        assert!(
            arr.iter()
                .any(|f| f["name"] == "hello.txt" && f["size"] == 5),
            "{files}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn restore_extracts_marker_next_to_volume() {
        let dir = temp_dir();
        let backup = instruction(
            "backup",
            json!({
                "server_id": "01234567-89ab-7def-8123-456789abcdef",
                "container_name": "pn-restore",
                "backup_id": "r1"
            }),
        );
        let backed = execute(&dir, &backup).await;
        assert!(backed.success, "{}", backed.message);
        let restore = instruction(
            "restore",
            json!({
                "server_id": "01234567-89ab-7def-8123-456789abcdef",
                "container_name": "pn-restore",
                "backup_id": "r1"
            }),
        );
        let result = execute(&dir, &restore).await;
        assert!(result.success, "{}", result.message);
        let marker = crate::docker::volume_host_dir(&dir, "pn-restore").join("MARKER");
        assert!(marker.is_file(), "expected {marker:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tar_roundtrip() {
        let tar = super::archive::write_files(&[("a.txt", b"abc")]).expect("tar");
        let gz = super::archive::gzip_wrap(&tar);
        let unpacked = super::archive::gzip_unwrap(&gz).expect("ungzip");
        let dest = temp_dir();
        super::archive::extract_tar(&unpacked, &dest).expect("extract");
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"abc");
        let _ = fs::remove_dir_all(&dest);
    }

    #[tokio::test]
    async fn delete_missing_container_cleans_volume() {
        let dir = temp_dir();
        let host = crate::docker::volume_host_dir(&dir, "pn-delete");
        fs::create_dir_all(&host).unwrap();
        fs::write(host.join("keep.txt"), b"x").unwrap();
        let job = instruction(
            "delete",
            json!({
                "server_id": "01234567-89ab-7def-8123-456789abcdef",
                "container_name": "pn-delete"
            }),
        );
        let result = execute(&dir, &job).await;
        assert!(result.success, "{}", result.message);
        assert_eq!(result.message, "deleted pn-delete");
        assert!(!host.exists(), "volume dir should be removed");
        let _ = fs::remove_dir_all(&dir);
    }
}
