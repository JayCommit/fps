//! systemd unit and env-file rendering for host install.
//!
//! This module never SSHes into Proxmox and never starts services. Guest
//! create stays in `apply`. Operators copy artifacts onto Fry/Homer and run
//! `deploy/install/install.sh` (default: do not start; pass `--start` to enable).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

pub const CONTROL_PLANE_UNIT_NAME: &str = "fps-control-plane.service";
pub const AGENT_UNIT_NAME: &str = "fps-node-agent.service";
pub const CONTROL_PLANE_ENV_NAME: &str = "control-plane.env.example";
pub const AGENT_ENV_NAME: &str = "node-agent.env.example";

#[derive(Debug, Clone, Serialize)]
pub struct InstallPlan {
    pub summary: String,
    pub units: Vec<String>,
    pub env_files: Vec<String>,
    pub binary_dest: String,
    pub notes: Vec<String>,
}

pub fn install_plan() -> InstallPlan {
    InstallPlan {
        summary: "Write systemd units and env templates. Does not start services, does not SSH to Proxmox, does not create guests.".into(),
        units: vec![
            CONTROL_PLANE_UNIT_NAME.to_string(),
            AGENT_UNIT_NAME.to_string(),
        ],
        env_files: vec![
            CONTROL_PLANE_ENV_NAME.to_string(),
            AGENT_ENV_NAME.to_string(),
        ],
        binary_dest: "/opt/fps/current".into(),
        notes: vec![
            "Copy fps-control-plane and fps-node-agent into /opt/fps/current/ when binaries exist.".into(),
            "Edit /etc/fps/*.env before the first start.".into(),
            "deploy/install/install.sh does not start units unless --start is passed.".into(),
            "This renderer never contacts Proxmox.".into(),
        ],
    }
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
pub fn write_install_artifacts(dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("create install artifact dir {}", dir.display()))?;

    fs::write(
        dir.join(CONTROL_PLANE_UNIT_NAME),
        render_control_plane_unit(),
    )?;
    fs::write(dir.join(AGENT_UNIT_NAME), render_agent_unit())?;
    fs::write(dir.join(CONTROL_PLANE_ENV_NAME), render_control_plane_env())?;
    fs::write(dir.join(AGENT_ENV_NAME), render_agent_env())?;

    let plan = install_plan();
    fs::write(
        dir.join("install-plan.json"),
        serde_json::to_vec_pretty(&plan)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pn-install-artifacts-{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        write_install_artifacts(&dir).unwrap();
        assert!(dir.join(CONTROL_PLANE_UNIT_NAME).is_file());
        assert!(dir.join(AGENT_UNIT_NAME).is_file());
        assert!(dir.join(CONTROL_PLANE_ENV_NAME).is_file());
        assert!(dir.join(AGENT_ENV_NAME).is_file());
        let plan: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("install-plan.json")).unwrap()).unwrap();
        assert_eq!(plan["binary_dest"], "/opt/fps/current");
        let _ = fs::remove_dir_all(&dir);
    }
}
