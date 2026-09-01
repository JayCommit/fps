use serde::Serialize;

use crate::config::{BootstrapConfig, GuestKind};

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentPlan {
    pub summary: String,
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

pub fn build_plan(cfg: &BootstrapConfig, dry_run: bool) -> DeploymentPlan {
    let mut actions = vec![
        PlanAction {
            host: cfg.control_plane.proxmox.node.clone(),
            action: match cfg.control_plane.guest_kind {
                GuestKind::Lxc => "create-lxc".into(),
                GuestKind::Vm => "create-vm".into(),
            },
            detail: format!(
                "vmid={} hostname={} cores={} memory_mib={} disk_gib={} storage={} bridge={}",
                cfg.control_plane.vmid,
                cfg.control_plane.hostname,
                cfg.control_plane.cores,
                cfg.control_plane.memory_mib,
                cfg.control_plane.disk_gib,
                cfg.control_plane.storage,
                cfg.control_plane.bridge
            ),
            mutating: true,
        },
        PlanAction {
            host: cfg.game_node.proxmox.node.clone(),
            action: "create-vm".into(),
            detail: format!(
                "vmid={} hostname={} cores={} memory_mib={} disk_gib={} storage={} bridge={} (Docker + node agent)",
                cfg.game_node.vmid,
                cfg.game_node.hostname,
                cfg.game_node.cores,
                cfg.game_node.memory_mib,
                cfg.game_node.disk_gib,
                cfg.game_node.storage,
                cfg.game_node.bridge
            ),
            mutating: true,
        },
        PlanAction {
            host: cfg.control_plane.hostname.clone(),
            action: "install-control-plane".into(),
            detail: "Install systemd units, run migrations, create owner via setup API".into(),
            mutating: true,
        },
        PlanAction {
            host: cfg.game_node.hostname.clone(),
            action: "enroll-node".into(),
            detail: "Issue a one-time enrollment token and start the node agent".into(),
            mutating: true,
        },
    ];
    if dry_run {
        for action in &mut actions {
            action.mutating = false;
            action.action = format!("plan:{}", action.action);
        }
    }
    DeploymentPlan {
        summary: format!(
            "Provision {} on {} and {} on {}",
            cfg.control_plane.hostname,
            cfg.control_plane.proxmox.node,
            cfg.game_node.hostname,
            cfg.game_node.proxmox.node
        ),
        warnings: vec![
            "Repeated apply is idempotent: existing VMIDs are never overwritten on collision.".into(),
            "OPNsense is not mutated by this release. Apply the generated firewall notes manually.".into(),
        ],
        opnsense_follow_up: vec![
            format!(
                "Allow the control plane ({}) to accept TCP 47890 from administrators and TCP 47891 from game nodes.",
                cfg.control_plane.ip_cidr
            ),
            format!(
                "Allow the game node ({}) egress to the control plane node port 47891/443 and required game ports as allocated.",
                cfg.game_node.ip_cidr
            ),
            "Site-to-site WireGuard between opn02 (Fry) and opn01 (Homer) must already route the guest subnets.".into(),
        ],
        actions,
    }
}
