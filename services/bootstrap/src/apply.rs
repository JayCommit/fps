use anyhow::Result;
use serde_json::{json, Value};

use crate::config::{BootstrapConfig, GuestKind, GuestSpec};
use crate::proxmox::ProxmoxView;

pub async fn apply_guests(
    cfg: &BootstrapConfig,
    control: &dyn ProxmoxView,
    game: &dyn ProxmoxView,
) -> Result<Vec<String>> {
    let mut upids = Vec::new();
    upids.push(create_guest(control, &cfg.control_plane).await?);
    upids.push(create_guest(game, &cfg.game_node).await?);
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
