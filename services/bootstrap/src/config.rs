use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::role::InstallRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub schema_version: u32,
    pub product_channel: String,
    /// Fry: web panel + API guest. Omit for a Homer-only file.
    #[serde(default)]
    pub control_plane: Option<GuestSpec>,
    /// Homer: game-node VM. Omit for a Fry-only file.
    #[serde(default)]
    pub game_node: Option<GuestSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestSpec {
    pub proxmox: ProxmoxEndpoint,
    pub guest_kind: GuestKind,
    pub vmid: u32,
    pub hostname: String,
    pub cores: u32,
    pub memory_mib: u32,
    pub disk_gib: u32,
    pub storage: String,
    pub bridge: String,
    pub vlan_tag: Option<u16>,
    pub ip_cidr: String,
    pub gateway: String,
    pub dns: Vec<String>,
    pub ssh_public_key: String,
    pub os_template: String,
    pub os_template_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestKind {
    Lxc,
    Vm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxmoxEndpoint {
    pub url: String,
    pub node: String,
    pub token_id: String,
    /// Environment variable that holds the token secret. Never the secret itself.
    pub token_secret_env: String,
}

impl BootstrapConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let cfg: Self = toml::from_str(&raw).context("parse deployment.toml")?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!(
                "unsupported bootstrap schema_version {}",
                self.schema_version
            );
        }
        if self.control_plane.is_none() && self.game_node.is_none() {
            bail!("deployment.toml must define [control_plane], [game_node], or both");
        }
        for guest in self.control_plane.iter().chain(self.game_node.iter()) {
            if guest.vmid < 100 {
                bail!("VMID {} is reserved by Proxmox (< 100)", guest.vmid);
            }
            if guest.cores == 0 || guest.memory_mib < 512 {
                bail!("{} has insufficient CPU or memory", guest.hostname);
            }
            if guest.ssh_public_key.contains("BEGIN") || guest.ssh_public_key.contains("PRIVATE") {
                bail!("ssh_public_key must be a public key, not a private key");
            }
            if guest.os_template_sha256.len() != 64 {
                bail!("os_template_sha256 must be a 64-character hex digest");
            }
            url::Url::parse(&guest.proxmox.url).context("proxmox url")?;
        }
        if let (Some(cp), Some(gn)) = (&self.control_plane, &self.game_node) {
            if cp.vmid == gn.vmid && cp.proxmox.node == gn.proxmox.node {
                bail!("control plane and game node VMIDs collide on the same Proxmox node");
            }
        }
        if let Some(gn) = &self.game_node {
            match gn.guest_kind {
                GuestKind::Vm => {}
                GuestKind::Lxc => {
                    bail!("game-node runtime must be a full VM, not LXC");
                }
            }
        }
        Ok(())
    }

    /// Guests this role will preflight or create. Fails if the file is missing that side.
    pub fn require_for_role(&self, role: InstallRole) -> Result<()> {
        if role.includes_control_plane() && self.control_plane.is_none() {
            bail!("--role {role} needs a [control_plane] section in the deployment file");
        }
        if role.includes_game_host() && self.game_node.is_none() {
            bail!("--role {role} needs a [game_node] section in the deployment file");
        }
        Ok(())
    }

    pub fn redacted(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "note".into(),
                serde_json::json!("token secrets are referenced by environment variable name only"),
            );
        }
        value
    }
}
