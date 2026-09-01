use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config::{BootstrapConfig, GuestKind, GuestSpec};
use crate::proxmox::ProxmoxView;
use crate::role::InstallRole;

pub async fn apply_guests(
    cfg: &BootstrapConfig,
    control: &dyn ProxmoxView,
    game: &dyn ProxmoxView,
    role: InstallRole,
) -> Result<Vec<String>> {
    cfg.require_for_role(role)?;
    let mut upids = Vec::new();
    if role.includes_control_plane() {
        let Some(cp) = &cfg.control_plane else {
            bail!("control-plane guest missing from config");
        };
        upids.push(create_guest(control, cp).await?);
    }
    if role.includes_game_host() {
        let Some(gn) = &cfg.game_node else {
            bail!("game-host guest missing from config");
        };
        upids.push(create_guest(game, gn).await?);
    }
    Ok(upids)
}

async fn create_guest(client: &dyn ProxmoxView, guest: &GuestSpec) -> Result<String> {
    match guest.guest_kind {
        GuestKind::Lxc => {
            client
                .create_lxc(&guest.proxmox.node, lxc_body(guest))
                .await
        }
        GuestKind::Vm => {
            client
                .create_qemu(&guest.proxmox.node, qemu_body(guest))
                .await
        }
    }
}

fn lxc_body(guest: &GuestSpec) -> Value {
    json!({
        "vmid": guest.vmid,
        "hostname": guest.hostname,
        "cores": guest.cores,
        "memory": guest.memory_mib,
        "ostemplate": guest.os_template,
        "rootfs": format!("{}:{}", guest.storage, guest.disk_gib),
        "net0": format!("name=eth0,bridge={},ip={}", guest.bridge, guest.ip_cidr),
        "ssh-public-keys": guest.ssh_public_key,
        "unprivileged": 1,
        "start": 0,
    })
}

fn qemu_body(guest: &GuestSpec) -> Value {
    json!({
        "vmid": guest.vmid,
        "name": guest.hostname,
        "cores": guest.cores,
        "memory": guest.memory_mib,
        "scsi0": format!("{}:{},size={}G", guest.storage, guest.vmid, guest.disk_gib),
        "net0": format!("virtio,bridge={}", guest.bridge),
        "scsihw": "virtio-scsi-pci",
        "ostype": "l26",
        "ciuser": "debian",
        "sshkeys": guest.os_template,
        "start": 0,
    })
}
