use std::process::Command;

fn proxmox_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/proxmox")
}

fn script(name: &str) -> std::path::PathBuf {
    proxmox_dir().join(name)
}

fn bash_n(path: &std::path::Path) {
    let status = Command::new("bash")
        .args(["-n", path.to_str().unwrap()])
        .status()
        .expect("bash -n");
    assert!(status.success(), "bash -n failed for {}", path.display());
}

fn run_install(args: &[&str]) -> (bool, String, String) {
    let output = Command::new("bash")
        .arg(script("install.sh").to_str().unwrap())
        .args(args)
        .output()
        .expect("run install.sh");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn proxmox_scripts_are_valid_bash() {
    for name in [
        "install.sh",
        "lib.sh",
        "guest-control-plane.sh",
        "guest-game-host.sh",
    ] {
        bash_n(&script(name));
    }
}

#[test]
fn install_sh_help_exits_zero() {
    let (ok, stdout, stderr) = run_install(&["--help"]);
    assert!(ok, "{stderr}");
    assert!(stdout.contains("control-plane"));
    assert!(stdout.contains("game-host"));
}

#[test]
fn dry_run_control_plane_uses_lxc_not_qemu() {
    let (ok, stdout, stderr) = run_install(&[
        "--dry-run",
        "--assume-proxmox",
        "--yes",
        "--role",
        "control-plane",
        "--vmid",
        "101",
        "--hostname",
        "fry",
        "--storage",
        "local-lvm",
        "--template-storage",
        "local",
        "--bridge",
        "vmbr0",
        "--ip",
        "dhcp",
    ]);
    assert!(ok, "stderr={stderr}\nstdout={stdout}");
    let combined = format!("{stdout}\n{stderr}");
    assert!(combined.contains("pct create"), "{combined}");
    assert!(combined.contains("101"), "{combined}");
    assert!(
        combined.contains("guest-control-plane") || combined.contains("fps-guest-bootstrap"),
        "{combined}"
    );
    assert!(
        !combined.contains("qm create"),
        "control plane dry-run should not create a QEMU VM: {combined}"
    );
}

#[test]
fn dry_run_game_host_uses_qemu_not_lxc() {
    let (ok, stdout, stderr) = run_install(&[
        "--dry-run",
        "--assume-proxmox",
        "--yes",
        "--role",
        "game-host",
        "--vmid",
        "201",
        "--hostname",
        "homer",
        "--storage",
        "local-lvm",
        "--template-storage",
        "local",
        "--bridge",
        "vmbr0",
        "--ip",
        "dhcp",
    ]);
    assert!(ok, "stderr={stderr}\nstdout={stdout}");
    let combined = format!("{stdout}\n{stderr}");
    assert!(combined.contains("qm create"), "{combined}");
    assert!(combined.contains("201"), "{combined}");
    assert!(
        combined.contains("cicustom")
            || combined.contains("cloud-init")
            || combined.contains("snippets"),
        "{combined}"
    );
    assert!(
        !combined.contains("pct create"),
        "game host dry-run should not create LXC: {combined}"
    );
}

#[test]
fn refuses_existing_vmid() {
    let (ok, stdout, stderr) = run_install(&[
        "--dry-run",
        "--assume-proxmox",
        "--yes",
        "--role",
        "control-plane",
        "--vmid",
        "101",
        "--existing-vmids",
        "100,101,102",
        "--hostname",
        "fry",
        "--storage",
        "local-lvm",
        "--ip",
        "dhcp",
    ]);
    assert!(!ok, "expected failure, stdout={stdout}");
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("already exists") || combined.contains("never overwrites"),
        "{combined}"
    );
}

#[test]
fn refuses_lxc_game_host() {
    let (ok, stdout, stderr) = run_install(&[
        "--dry-run",
        "--assume-proxmox",
        "--yes",
        "--role",
        "game-host",
        "--guest-type",
        "lxc",
        "--vmid",
        "202",
        "--ip",
        "dhcp",
    ]);
    assert!(!ok, "expected failure, stdout={stdout}");
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.to_lowercase().contains("lxc") && combined.to_lowercase().contains("vm"),
        "{combined}"
    );
}

#[test]
fn guest_scripts_dry_run() {
    for name in ["guest-control-plane.sh", "guest-game-host.sh"] {
        let output = Command::new("bash")
            .arg(script(name).to_str().unwrap())
            .arg("--dry-run")
            .output()
            .expect("guest dry-run");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{} failed: {stderr}\n{stdout}",
            name
        );
        assert!(
            stdout.contains("git clone") || stdout.contains("+ git"),
            "{name}: {stdout}"
        );
    }
}

#[test]
fn control_plane_guest_mentions_web_and_mariadb() {
    let body = std::fs::read_to_string(script("guest-control-plane.sh")).unwrap();
    assert!(body.contains("mariadb"));
    assert!(body.contains("pnpm"));
    assert!(body.contains("FPS_WEB_ROOT"));
    assert!(body.contains("fps-control-plane"));
}

#[test]
fn game_host_guest_mentions_docker_and_agent() {
    let body = std::fs::read_to_string(script("guest-game-host.sh")).unwrap();
    assert!(body.contains("docker-ce"));
    assert!(body.contains("fps-node-agent"));
    assert!(body.contains("full VMs") || body.contains("full VM"));
}
