//! Host install for a chosen role: control plane, game host, or both.
//!
//! This module never SSHes into Proxmox and never creates guests. Guest create
//! stays in `apply`. Default is not to start systemd units (`--start` to enable).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::role::InstallRole;

pub const CONTROL_PLANE_UNIT_NAME: &str = "fps-control-plane.service";
pub const AGENT_UNIT_NAME: &str = "fps-node-agent.service";
pub const CONTROL_PLANE_ENV_NAME: &str = "control-plane.env.example";
pub const AGENT_ENV_NAME: &str = "node-agent.env.example";

#[derive(Debug, Clone, Serialize)]
pub struct InstallPlan {
    pub summary: String,
    pub role: InstallRole,
    pub units: Vec<String>,
    pub env_files: Vec<String>,
    pub binary_dest: String,
    pub notes: Vec<String>,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostInstallReport {
    pub role: InstallRole,
    pub prefix: String,
    pub wrote: Vec<String>,
    pub skipped: Vec<String>,
    pub started: bool,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HostInstallOpts {
    pub role: InstallRole,
    pub start: bool,
    pub destdir: Option<PathBuf>,
    pub bin_dir: Option<PathBuf>,
    pub prefix: PathBuf,
}

impl Default for HostInstallOpts {
    fn default() -> Self {
        Self {
            role: InstallRole::Both,
            start: false,
            destdir: None,
            bin_dir: None,
            prefix: PathBuf::from("/opt/fps"),
        }
    }
}

pub fn install_plan(role: InstallRole) -> InstallPlan {
    let mut units = Vec::new();
    let mut env_files = Vec::new();
    if role.includes_control_plane() {
        units.push(CONTROL_PLANE_UNIT_NAME.to_string());
        env_files.push(CONTROL_PLANE_ENV_NAME.to_string());
    }
    if role.includes_game_host() {
        units.push(AGENT_UNIT_NAME.to_string());
        env_files.push(AGENT_ENV_NAME.to_string());
    }
    InstallPlan {
        summary: format!(
            "Write systemd units for {}. Does not start services, does not SSH to Proxmox, does not create guests.",
            role.title()
        ),
        role,
        units,
        env_files,
        binary_dest: "/opt/fps/current".into(),
        notes: vec![
            "Copy matching binaries into /opt/fps/current/ when they exist.".into(),
            "Edit /etc/fps/*.env before the first start.".into(),
            "Pass --start to enable systemd units. Default is write-only.".into(),
            "This installer never contacts Proxmox.".into(),
        ],
        next_steps: next_steps(role),
    }
}

pub fn next_steps(role: InstallRole) -> Vec<String> {
    let mut steps = Vec::new();
    if role.includes_control_plane() {
        steps.push("Edit /etc/fps/control-plane.env: MariaDB URL and FPS_MASTER_KEY (openssl rand -hex 32).".into());
        steps.push("Start MariaDB, then: systemctl enable --now fps-control-plane (or re-run with --start).".into());
        steps
            .push("Open the web UI and create the owner (password at least 12 characters).".into());
    }
    if role.includes_game_host() {
        steps.push(
            "Install Docker Engine on this host (full VM, overlayfs). Never LXC for games.".into(),
        );
        steps.push("In the FPS UI: Nodes → create an enrollment token.".into());
        steps.push("fps-node-agent enroll --url https://CONTROL_PLANE:47890 --token TOKEN --data-dir /var/lib/fps/agent".into());
        steps.push("systemctl enable --now fps-node-agent (or re-run with --start).".into());
    }
    if role == InstallRole::Both {
        steps.push(
            "Both roles on one machine is for labs. Production is Fry (web) + Homer (games)."
                .into(),
        );
    }
    steps
}

pub fn render_control_plane_unit() -> String {
    include_str!("../../../deploy/systemd/fps-control-plane.service").to_string()
}

pub fn render_agent_unit() -> String {
    include_str!("../../../deploy/systemd/fps-node-agent.service").to_string()
}

pub fn render_control_plane_env() -> String {
    r#"# FPS control plane environment
# Install to /etc/fps/control-plane.env and restrict to 0600.

FPS_DATABASE_URL=mysql://fps:change-me@127.0.0.1:3306/fps
FPS_MASTER_KEY=
FPS_HTTP_BIND=0.0.0.0:47890
FPS_NODE_BIND=0.0.0.0:47891
FPS_PUBLIC_URL=http://127.0.0.1:47890
FPS_DATA_DIR=/var/lib/fps
FPS_ALLOW_INSECURE_HTTP=false
FPS_LOG_FORMAT=json
"#
    .to_string()
}

pub fn render_agent_env() -> String {
    r#"# FPS node agent environment
# Install to /etc/fps/node-agent.env and restrict to 0600.

# Enrollment is a separate command (`fps-node-agent enroll ...`).
# This file is for the long-running `run` service only.
FPS_LOG_FORMAT=json
"#
    .to_string()
}

/// Write units, env examples, and `install-plan.json` under `dir`.
pub fn write_install_artifacts(dir: impl AsRef<Path>, role: InstallRole) -> Result<()> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("create install artifact dir {}", dir.display()))?;

    if role.includes_control_plane() {
        fs::write(
            dir.join(CONTROL_PLANE_UNIT_NAME),
            render_control_plane_unit(),
        )?;
        fs::write(dir.join(CONTROL_PLANE_ENV_NAME), render_control_plane_env())?;
    }
    if role.includes_game_host() {
        fs::write(dir.join(AGENT_UNIT_NAME), render_agent_unit())?;
        fs::write(dir.join(AGENT_ENV_NAME), render_agent_env())?;
    }

    let plan = install_plan(role);
    fs::write(
        dir.join("install-plan.json"),
        serde_json::to_vec_pretty(&plan)?,
    )?;
    Ok(())
}

fn join_root(destdir: Option<&Path>, path: &Path) -> PathBuf {
    match destdir {
        Some(root) => {
            let rel = path.strip_prefix("/").unwrap_or(path);
            root.join(rel)
        }
        None => path.to_path_buf(),
    }
}

fn copy_bin_if_present(
    name: &str,
    dest_dir: &Path,
    bin_dir: Option<&Path>,
    wrote: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let dest = dest_dir.join(name);
    let mut src: Option<PathBuf> = None;
    if let Some(dir) = bin_dir {
        let candidate = dir.join(name);
        if candidate.is_file() {
            src = Some(candidate);
        }
    }
    if src.is_none() {
        let cwd = PathBuf::from(name);
        if cwd.is_file() {
            src = Some(cwd);
        }
    }
    if let Some(src) = src {
        fs::create_dir_all(dest_dir)?;
        fs::copy(&src, &dest)
            .with_context(|| format!("copy {} -> {}", src.display(), dest.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
        }
        wrote.push(format!("{} -> {}", src.display(), dest.display()));
    } else {
        skipped.push(format!(
            "binary {name} not found (place it at {} later)",
            dest.display()
        ));
    }
    Ok(())
}

fn write_env_if_missing(
    path: &Path,
    body: &str,
    wrote: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    if path.exists() {
        skipped.push(format!("keep existing {}", path.display()));
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    wrote.push(path.display().to_string());
    Ok(())
}

/// Install units and env files onto this machine (or under `--destdir` for tests).
pub fn perform_host_install(opts: &HostInstallOpts) -> Result<HostInstallReport> {
    let dest_root = opts.destdir.as_deref();
    let current = join_root(dest_root, &opts.prefix.join("current"));
    let unitdir = join_root(dest_root, Path::new("/etc/systemd/system"));
    let envdir = join_root(dest_root, Path::new("/etc/fps"));
    let mut wrote = Vec::new();
    let mut skipped = Vec::new();

    fs::create_dir_all(&current)?;
    fs::create_dir_all(&unitdir)?;
    fs::create_dir_all(&envdir)?;

    copy_bin_if_present(
        "fps",
        &current,
        opts.bin_dir.as_deref(),
        &mut wrote,
        &mut skipped,
    )?;

    if opts.role.includes_control_plane() {
        fs::create_dir_all(join_root(dest_root, Path::new("/var/lib/fps")))?;
        copy_bin_if_present(
            "fps-control-plane",
            &current,
            opts.bin_dir.as_deref(),
            &mut wrote,
            &mut skipped,
        )?;
        let unit = unitdir.join(CONTROL_PLANE_UNIT_NAME);
        fs::write(&unit, render_control_plane_unit())?;
        wrote.push(unit.display().to_string());
        write_env_if_missing(
            &envdir.join("control-plane.env"),
            &render_control_plane_env(),
            &mut wrote,
            &mut skipped,
        )?;
    }

    if opts.role.includes_game_host() {
        fs::create_dir_all(join_root(dest_root, Path::new("/var/lib/fps/agent")))?;
        copy_bin_if_present(
            "fps-node-agent",
            &current,
            opts.bin_dir.as_deref(),
            &mut wrote,
            &mut skipped,
        )?;
        let unit = unitdir.join(AGENT_UNIT_NAME);
        fs::write(&unit, render_agent_unit())?;
        wrote.push(unit.display().to_string());
        write_env_if_missing(
            &envdir.join("node-agent.env"),
            &render_agent_env(),
            &mut wrote,
            &mut skipped,
        )?;
    }

    let start_now = opts.start && opts.destdir.is_none();
    if start_now {
        start_units(opts.role)?;
    } else if opts.start && opts.destdir.is_some() {
        skipped.push("destdir set; not invoking systemctl".into());
    }

    Ok(HostInstallReport {
        role: opts.role,
        prefix: opts.prefix.display().to_string(),
        wrote,
        skipped,
        started: start_now,
        next_steps: next_steps(opts.role),
    })
}

fn start_units(role: InstallRole) -> Result<()> {
    let mut cmd = Command::new("systemctl");
    cmd.arg("daemon-reload");
    let status = cmd.status().context("systemctl daemon-reload")?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }
    if role.includes_control_plane() {
        enable_now("fps-control-plane.service")?;
    }
    if role.includes_game_host() {
        enable_now("fps-node-agent.service")?;
    }
    Ok(())
}

fn enable_now(unit: &str) -> Result<()> {
    let status = Command::new("systemctl")
        .args(["enable", "--now", unit])
        .status()
        .with_context(|| format!("systemctl enable --now {unit}"))?;
    if !status.success() {
        anyhow::bail!("systemctl enable --now {unit} failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("fps-install-{label}-{stamp}"))
    }

    #[test]
    fn units_are_systemd_and_never_ssh_proxmox() {
        let cp = render_control_plane_unit();
        let agent = render_agent_unit();
        for body in [&cp, &agent] {
            assert!(body.contains("[Service]"));
            assert!(body.contains("ExecStart="));
            assert!(!body.to_lowercase().contains("ssh "));
            assert!(!body.to_lowercase().contains("proxmox"));
        }
        assert!(cp.contains("fps-control-plane serve"));
        assert!(agent.contains("fps-node-agent run"));
    }

    #[test]
    fn write_install_artifacts_creates_plan_and_units() {
        let dir = scratch("both");
        let _ = fs::remove_dir_all(&dir);
        write_install_artifacts(&dir, InstallRole::Both).unwrap();
        assert!(dir.join(CONTROL_PLANE_UNIT_NAME).is_file());
        assert!(dir.join(AGENT_UNIT_NAME).is_file());
        assert!(dir.join(CONTROL_PLANE_ENV_NAME).is_file());
        assert!(dir.join(AGENT_ENV_NAME).is_file());
        let plan: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("install-plan.json")).unwrap()).unwrap();
        assert_eq!(plan["binary_dest"], "/opt/fps/current");
        assert_eq!(plan["role"], "both");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn control_plane_artifacts_omit_agent() {
        let dir = scratch("cp");
        write_install_artifacts(&dir, InstallRole::ControlPlane).unwrap();
        assert!(dir.join(CONTROL_PLANE_UNIT_NAME).is_file());
        assert!(!dir.join(AGENT_UNIT_NAME).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn game_host_artifacts_omit_control_plane() {
        let dir = scratch("gh");
        write_install_artifacts(&dir, InstallRole::GameHost).unwrap();
        assert!(dir.join(AGENT_UNIT_NAME).is_file());
        assert!(!dir.join(CONTROL_PLANE_UNIT_NAME).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn host_install_game_host_under_destdir() {
        let dest = scratch("destdir");
        let bins = scratch("bins");
        fs::create_dir_all(&bins).unwrap();
        fs::write(bins.join("fps-node-agent"), b"#!/bin/true\n").unwrap();
        let report = perform_host_install(&HostInstallOpts {
            role: InstallRole::GameHost,
            start: true,
            destdir: Some(dest.clone()),
            bin_dir: Some(bins.clone()),
            prefix: PathBuf::from("/opt/fps"),
        })
        .unwrap();
        assert!(!report.started);
        assert!(dest
            .join("etc/systemd/system")
            .join(AGENT_UNIT_NAME)
            .is_file());
        assert!(!dest
            .join("etc/systemd/system")
            .join(CONTROL_PLANE_UNIT_NAME)
            .exists());
        assert!(dest.join("opt/fps/current/fps-node-agent").is_file());
        assert!(dest.join("etc/fps/node-agent.env").is_file());
        assert!(!dest.join("etc/fps/control-plane.env").exists());
        let _ = fs::remove_dir_all(&dest);
        let _ = fs::remove_dir_all(&bins);
    }
}
