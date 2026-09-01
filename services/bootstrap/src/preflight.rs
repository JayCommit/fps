use anyhow::Result;
use serde::Serialize;

use crate::config::BootstrapConfig;
use crate::proxmox::ProxmoxView;

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightReport {
    pub ok: bool,
    pub checks: Vec<Check>,
}

pub async fn run_preflight(
    cfg: &BootstrapConfig,
    control: &dyn ProxmoxView,
    game: &dyn ProxmoxView,
) -> Result<PreflightReport> {
    let mut checks = Vec::new();
    checks.extend(inspect_host("control-plane", &cfg.control_plane, control).await?);
    checks.extend(inspect_host("game-node", &cfg.game_node, game).await?);
    if cfg.control_plane.vmid == cfg.game_node.vmid
        && cfg.control_plane.proxmox.node == cfg.game_node.proxmox.node
    {
        checks.push(Check {
            name: "vmid-collision".into(),
            ok: false,
            detail: "control-plane and game-node share a VMID on the same node".into(),
        });
    }
    let ok = checks.iter().all(|c| c.ok);
    Ok(PreflightReport { ok, checks })
}

async fn inspect_host(
    role: &str,
    guest: &crate::config::GuestSpec,
    client: &dyn ProxmoxView,
) -> Result<Vec<Check>> {
    let mut checks = Vec::new();
    match client.version().await {
        Ok(v) => checks.push(Check {
            name: format!("{role}.version"),
            ok: v.starts_with('8') || v.starts_with('9'),
            detail: format!("Proxmox version {v}"),
        }),
        Err(e) => checks.push(Check {
            name: format!("{role}.version"),
            ok: false,
            detail: e.to_string(),
        }),
    }
    match client.node_online(&guest.proxmox.node).await {
        Ok(true) => checks.push(Check {
            name: format!("{role}.node"),
            ok: true,
            detail: format!("node {} online", guest.proxmox.node),
        }),
        Ok(false) => checks.push(Check {
            name: format!("{role}.node"),
            ok: false,
            detail: format!("node {} is not online", guest.proxmox.node),
        }),
        Err(e) => checks.push(Check {
            name: format!("{role}.node"),
            ok: false,
            detail: e.to_string(),
        }),
    }
    match client
        .has_storage(&guest.proxmox.node, &guest.storage)
        .await
    {
        Ok(true) => checks.push(Check {
            name: format!("{role}.storage"),
            ok: true,
            detail: guest.storage.clone(),
        }),
        Ok(false) => checks.push(Check {
            name: format!("{role}.storage"),
            ok: false,
            detail: format!("storage '{}' not found", guest.storage),
        }),
        Err(e) => checks.push(Check {
            name: format!("{role}.storage"),
            ok: false,
            detail: e.to_string(),
        }),
    }
    match client.has_bridge(&guest.proxmox.node, &guest.bridge).await {
        Ok(true) => checks.push(Check {
            name: format!("{role}.bridge"),
            ok: true,
            detail: guest.bridge.clone(),
        }),
        Ok(false) => checks.push(Check {
            name: format!("{role}.bridge"),
            ok: false,
            detail: format!("bridge '{}' not found", guest.bridge),
        }),
        Err(e) => checks.push(Check {
            name: format!("{role}.bridge"),
            ok: false,
            detail: e.to_string(),
        }),
    }
    match client.vmid_in_use(guest.vmid).await {
        Ok(true) => checks.push(Check {
            name: format!("{role}.vmid"),
            ok: false,
            detail: format!(
                "VMID {} already exists; refusing to overwrite. Choose a free id.",
                guest.vmid
            ),
        }),
        Ok(false) => checks.push(Check {
            name: format!("{role}.vmid"),
            ok: true,
            detail: format!("VMID {} is free", guest.vmid),
        }),
        Err(e) => checks.push(Check {
            name: format!("{role}.vmid"),
            ok: false,
            detail: e.to_string(),
        }),
    }
    Ok(checks)
}
