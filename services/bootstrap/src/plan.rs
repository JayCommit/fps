use serde::Serialize;

use crate::config::{BootstrapConfig, GuestKind};
use crate::role::InstallRole;

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentPlan {
    pub summary: String,
    pub role: InstallRole,
    pub actions: Vec<PlanAction>,
    pub warnings: Vec<String>,
    pub opnsense_follow_up: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanAction {
    pub host: String,
    pub action: String,
    pub detail: String,
    pub mutating: bool,
}

pub fn build_plan(cfg: &BootstrapConfig, dry_run: bool, role: InstallRole) -> DeploymentPlan {
    let mut actions = Vec::new();
    if role.includes_control_plane() {
        if let Some(cp) = &cfg.control_plane {
            actions.push(PlanAction {
                host: cp.proxmox.node.clone(),
                action: match cp.guest_kind {
                    GuestKind::Lxc => "create-lxc".into(),
                    GuestKind::Vm => "create-vm".into(),
                },
                detail: format!(
                    "vmid={} hostname={} cores={} memory_mib={} disk_gib={} storage={} bridge={}",
                    cp.vmid,
                    cp.hostname,
                    cp.cores,
                    cp.memory_mib,
                    cp.disk_gib,
                    cp.storage,
                    cp.bridge
                ),
                mutating: true,
            });
            actions.push(PlanAction {
                host: cp.hostname.clone(),
                action: "install-control-plane".into(),
                detail: "On the guest: fps install --role control-plane, then create the owner in the UI"
                    .into(),
                mutating: true,
            });
        }
    }
    if role.includes_game_host() {
        if let Some(gn) = &cfg.game_node {
            actions.push(PlanAction {
                host: gn.proxmox.node.clone(),
                action: "create-vm".into(),
                detail: format!(
                    "vmid={} hostname={} cores={} memory_mib={} disk_gib={} storage={} bridge={} (Docker + node agent)",
                    gn.vmid, gn.hostname, gn.cores, gn.memory_mib, gn.disk_gib, gn.storage, gn.bridge
                ),
                mutating: true,
            });
            actions.push(PlanAction {
                host: gn.hostname.clone(),
                action: "install-game-host".into(),
                detail: "On the guest: fps install --role game-host, then enroll with a UI token"
                    .into(),
                mutating: true,
            });
        }
    }
    if dry_run {
        for action in &mut actions {
            action.mutating = false;
            action.action = format!("plan:{}", action.action);
        }
    }
    let summary = match role {
        InstallRole::ControlPlane => cfg.control_plane.as_ref().map(|g| {
            format!(
                "Provision control plane {} on {}",
                g.hostname, g.proxmox.node
            )
        }),
        InstallRole::GameHost => cfg
            .game_node
            .as_ref()
            .map(|g| format!("Provision game host {} on {}", g.hostname, g.proxmox.node)),
        InstallRole::Both => match (&cfg.control_plane, &cfg.game_node) {
            (Some(cp), Some(gn)) => Some(format!(
                "Provision {} on {} and {} on {}",
                cp.hostname, cp.proxmox.node, gn.hostname, gn.proxmox.node
            )),
            _ => Some("Provision the guests defined in the deployment file".into()),
        },
    }
    .unwrap_or_else(|| "No matching guests in the deployment file".into());

    DeploymentPlan {
        summary,
        role,
        warnings: vec![
            "Repeated apply is idempotent: existing VMIDs are never overwritten on collision."
                .into(),
            "OPNsense is not mutated by this release. Apply the generated firewall notes manually."
                .into(),
            "Guest create does not install FPS. SSH in and run `fps install` for that role.".into(),
        ],
        opnsense_follow_up: opnsense_notes(cfg, role),
        actions,
    }
}

fn opnsense_notes(cfg: &BootstrapConfig, role: InstallRole) -> Vec<String> {
    let mut notes = Vec::new();
    if role.includes_control_plane() {
        if let Some(cp) = &cfg.control_plane {
            notes.push(format!(
                "Allow the control plane ({}) to accept TCP 47890 from administrators and TCP 47891 from game nodes.",
                cp.ip_cidr
            ));
        }
    }
    if role.includes_game_host() {
        if let Some(gn) = &cfg.game_node {
            notes.push(format!(
                "Allow the game node ({}) egress to the control plane node port 47891 and required game ports as allocated.",
                gn.ip_cidr
            ));
        }
    }
    notes.push(
        "Site-to-site WireGuard between opn02 (Fry) and opn01 (Homer) must already route the guest subnets.".into(),
    );
    notes
}
