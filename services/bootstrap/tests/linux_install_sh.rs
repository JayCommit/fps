use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_install_sh() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/install.sh")
}

fn proxmox_stub() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/proxmox/install.sh")
}

fn bash_n(path: &Path) {
    let status = Command::new("bash")
        .args(["-n", path.to_str().unwrap()])
        .status()
        .expect("bash -n");
    assert!(status.success(), "bash -n failed for {}", path.display());
}

fn write_os_release(id: &str, version: &str, codename: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fps-os-release-{}-{}",
        id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("os-release");
    fs::write(
        &path,
        format!(
            "ID={id}\nVERSION_ID={version}\nVERSION_CODENAME={codename}\nPRETTY_NAME=\"{id} {version}\"\n"
        ),
    )
    .unwrap();
    path
}

fn run_install(args: &[&str]) -> (bool, String, String) {
    let output = Command::new("bash")
        .arg(repo_install_sh().to_str().unwrap())
        .args(args)
        .env_remove("FPS_TEST_ANSWERS")
        .env("FPS_FORCE_NO_TTY", "1")
        .stdin(Stdio::null())
        .output()
        .expect("run install.sh");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn dry_run(role: &str, os_release: &Path, extra: &[&str]) -> (bool, String) {
    let mut args = vec![
        "--dry-run",
        "--assume-root",
        "--yes",
        "--role",
        role,
        "--os-release-file",
        os_release.to_str().unwrap(),
        "--public-host",
        "10.0.0.8",
    ];
    args.extend_from_slice(extra);
    let (ok, stdout, stderr) = run_install(&args);
    let combined = format!("{stdout}\n{stderr}");
    (ok, combined)
}

#[test]
fn linux_scripts_are_valid_bash() {
    bash_n(&repo_install_sh());
    bash_n(&proxmox_stub());
    bash_n(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/install/install.sh"));
}

#[test]
fn install_sh_help_exits_zero() {
    let (ok, stdout, stderr) = run_install(&["--help"]);
    assert!(ok, "{stderr}");
    assert!(stdout.contains("control-plane"));
    assert!(stdout.contains("game-host"));
    assert!(stdout.contains("Ubuntu") || stdout.contains("Debian"));
    assert!(!stdout.to_lowercase().contains("pct create"));
    assert!(!stdout.to_lowercase().contains("creates a guest"));
    assert!(
        !stdout.contains("FPS_GITHUB_TOKEN") && !stdout.contains("ghp_"),
        "public install must not require a GitHub token: {stdout}"
    );
}

#[test]
fn proxmox_stub_points_at_linux_installer() {
    let output = Command::new("bash")
        .arg(proxmox_stub().to_str().unwrap())
        .output()
        .expect("proxmox stub");
    assert!(!output.status.success());
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("deploy/install.sh"), "{combined}");
    assert!(
        combined.to_lowercase().contains("ubuntu") || combined.to_lowercase().contains("debian"),
        "{combined}"
    );
    assert!(
        !combined.contains("FPS_GITHUB_TOKEN") && !combined.contains("ghp_"),
        "public install must not require a GitHub token: {combined}"
    );
}

#[test]
fn dry_run_control_plane_on_ubuntu() {
    let os = write_os_release("ubuntu", "24.04", "noble");
    let (ok, combined) = dry_run("control-plane", &os, &[]);
    assert!(ok, "{combined}");
    assert!(
        combined.contains("MariaDB") || combined.contains("mariadb"),
        "{combined}"
    );
    assert!(combined.contains("fps-control-plane"), "{combined}");
    assert!(
        combined.contains("pnpm") || combined.contains("Node.js"),
        "{combined}"
    );
    assert!(
        combined.contains("rustup") || combined.contains("Rust"),
        "{combined}"
    );
    assert!(combined.contains("10.0.0.8"), "{combined}");
    assert_builds_workspace_packages(&combined, &["fps-bootstrap", "fps-control-plane"]);
    assert!(
        !combined.contains("docker-ce"),
        "control plane should not install Docker Engine: {combined}"
    );
    assert!(!combined.contains("pct create"), "{combined}");
    assert!(!combined.contains("qm create"), "{combined}");
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn dry_run_game_host_uses_ubuntu_docker_repo() {
    let os = write_os_release("ubuntu", "24.04", "noble");
    let (ok, combined) = dry_run("game-host", &os, &[]);
    assert!(ok, "{combined}");
    assert!(
        combined.contains("download.docker.com/linux/ubuntu"),
        "{combined}"
    );
    assert!(combined.contains("fps-node-agent"), "{combined}");
    assert_builds_workspace_packages(&combined, &["fps-bootstrap", "fps-node-agent"]);
    assert!(combined.contains("noble"), "{combined}");
    assert!(
        !combined.contains("mariadb-server"),
        "game host should not install MariaDB: {combined}"
    );
    assert!(!combined.contains("pct create"), "{combined}");
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn dry_run_game_host_uses_debian_docker_repo() {
    let os = write_os_release("debian", "12", "bookworm");
    let (ok, combined) = dry_run("game-host", &os, &[]);
    assert!(ok, "{combined}");
    assert!(
        combined.contains("download.docker.com/linux/debian"),
        "{combined}"
    );
    assert!(combined.contains("bookworm"), "{combined}");
    assert_builds_workspace_packages(&combined, &["fps-bootstrap", "fps-node-agent"]);
    assert!(
        !combined.contains("download.docker.com/linux/ubuntu"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn dry_run_both_mentions_panel_and_docker() {
    let os = write_os_release("debian", "12", "bookworm");
    let (ok, combined) = dry_run("both", &os, &[]);
    assert!(ok, "{combined}");
    assert!(
        combined.contains("mariadb") || combined.contains("MariaDB"),
        "{combined}"
    );
    assert!(
        combined.contains("download.docker.com/linux/debian"),
        "{combined}"
    );
    assert!(combined.contains("fps-control-plane"), "{combined}");
    assert!(combined.contains("fps-node-agent"), "{combined}");
    assert_builds_workspace_packages(
        &combined,
        &["fps-bootstrap", "fps-control-plane", "fps-node-agent"],
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

fn assert_builds_workspace_packages(combined: &str, packages: &[&str]) {
    for pkg in packages {
        let flag = format!("-p {pkg}");
        assert!(
            combined.contains(&flag),
            "expected cargo -p {pkg} in installer plan:\n{combined}"
        );
    }
    // The binary is named `fps`; the Cargo package is `fps-bootstrap`.
    // `cargo build -p fps` fails with: package ID specification `fps` did not match.
    assert!(
        !combined.contains("-p fps ") && !combined.contains("-p fps\n"),
        "installer must not pass cargo package id `fps`:\n{combined}"
    );
}

#[test]
fn dry_run_ubuntu_26_game_host_uses_noble_docker_pocket() {
    let os = write_os_release("ubuntu", "26.04", "resolute");
    let (ok, combined) = dry_run("game-host", &os, &[]);
    assert!(ok, "{combined}");
    assert!(
        combined.contains("download.docker.com/linux/ubuntu"),
        "{combined}"
    );
    assert!(
        combined.contains(" noble "),
        "Ubuntu 26.04/resolute must use the Docker noble apt pocket: {combined}"
    );
    assert!(
        combined.contains("noble") && combined.to_lowercase().contains("docker"),
        "{combined}"
    );
    assert_builds_workspace_packages(&combined, &["fps-bootstrap", "fps-node-agent"]);
    assert!(
        !combined.contains("mariadb-server"),
        "game host should not install MariaDB: {combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn refuses_fedora() {
    let os = write_os_release("fedora", "41", "fortyone");
    let (ok, combined) = dry_run("control-plane", &os, &[]);
    assert!(!ok, "expected failure, got success: {combined}");
    assert!(
        combined.to_lowercase().contains("ubuntu") && combined.to_lowercase().contains("debian"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn refuses_old_ubuntu() {
    let os = write_os_release("ubuntu", "18.04", "bionic");
    let (ok, combined) = dry_run("control-plane", &os, &[]);
    assert!(!ok, "{combined}");
    assert!(
        combined.contains("too old") || combined.contains("22.04"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn unattended_without_role_explains_flag() {
    let os = write_os_release("ubuntu", "24.04", "noble");
    let (ok, stdout, stderr) = run_install(&[
        "--dry-run",
        "--assume-root",
        "--yes",
        "--os-release-file",
        os.to_str().unwrap(),
    ]);
    assert!(!ok);
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("--role") && combined.contains("unattended"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn missing_role_without_terminal_explains_the_menu() {
    let os = write_os_release("ubuntu", "24.04", "noble");
    let output = Command::new("bash")
        .env("FPS_FORCE_NO_TTY", "1")
        .env_remove("FPS_TEST_ANSWERS")
        .arg(repo_install_sh().to_str().unwrap())
        .args([
            "--dry-run",
            "--assume-root",
            "--os-release-file",
            os.to_str().unwrap(),
        ])
        .stdin(Stdio::null())
        .output()
        .expect("no-tty install.sh");
    assert!(!output.status.success());
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("role menu") || combined.contains("--role"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn piped_stdin_still_runs_when_role_is_passed() {
    let os = write_os_release("ubuntu", "24.04", "noble");
    let output = Command::new("bash")
        .arg(repo_install_sh().to_str().unwrap())
        .args([
            "--dry-run",
            "--assume-root",
            "--yes",
            "--role",
            "control-plane",
            "--os-release-file",
            os.to_str().unwrap(),
            "--public-host",
            "10.1.2.3",
        ])
        .stdin(Stdio::piped())
        .output()
        .expect("piped install.sh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "curl|bash with --role should work: {stderr}\n{stdout}"
    );
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("mariadb") || combined.contains("MariaDB"),
        "{combined}"
    );
    assert!(combined.contains("10.1.2.3"), "{combined}");
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn interactive_answers_select_control_plane() {
    let os = write_os_release("debian", "12", "bookworm");
    let output = Command::new("bash")
        .env("FPS_TEST_ANSWERS", "1,panel.example.test,y,y,y,y")
        .env_remove("FPS_FORCE_NO_TTY")
        .arg(repo_install_sh().to_str().unwrap())
        .args([
            "--dry-run",
            "--assume-root",
            "--os-release-file",
            os.to_str().unwrap(),
        ])
        .stdin(Stdio::null())
        .output()
        .expect("interactive install.sh");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{combined}");
    assert!(
        combined.contains("control-plane") || combined.contains("Control plane"),
        "{combined}"
    );
    assert!(combined.contains("panel.example.test"), "{combined}");
    assert!(
        combined.contains("mariadb") || combined.contains("MariaDB"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn no_mariadb_flag_skips_server_package() {
    let os = write_os_release("ubuntu", "24.04", "noble");
    let (ok, combined) = dry_run("control-plane", &os, &["--no-mariadb"]);
    assert!(ok, "{combined}");
    assert!(
        !combined.contains("apt-get install -y mariadb-server"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn enroll_flags_show_up_in_game_host_plan() {
    let os = write_os_release("ubuntu", "22.04", "jammy");
    let (ok, combined) = dry_run(
        "game-host",
        &os,
        &[
            "--control-plane-url",
            "http://10.0.0.8:47890",
            "--enroll-token",
            "test-token",
        ],
    );
    assert!(ok, "{combined}");
    assert!(combined.contains("fps-node-agent enroll"), "{combined}");
    assert!(combined.contains("http://10.0.0.8:47890"), "{combined}");
    assert!(combined.contains("jammy"), "{combined}");
    let _ = fs::remove_dir_all(os.parent().unwrap());
}

#[test]
fn dry_run_does_not_invoke_openssl() {
    let os = write_os_release("ubuntu", "22.04", "jammy");
    let trap = std::env::temp_dir().join(format!(
        "fps-no-openssl-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&trap).unwrap();
    fs::write(
        trap.join("openssl"),
        "#!/bin/sh\necho 'openssl must not run during --dry-run' >&2\nexit 97\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(trap.join("openssl"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:{}", trap.display(), std::env::var("PATH").unwrap());
    let output = Command::new("bash")
        .env("PATH", path)
        .env("FPS_FORCE_NO_TTY", "1")
        .env_remove("FPS_TEST_ANSWERS")
        .arg(repo_install_sh().to_str().unwrap())
        .args([
            "--dry-run",
            "--assume-root",
            "--yes",
            "--role",
            "both",
            "--os-release-file",
            os.to_str().unwrap(),
            "--public-host",
            "10.0.0.8",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("dry-run without openssl");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "dry-run must not call openssl: {combined}"
    );
    assert!(
        combined.contains("mariadb") || combined.contains("MariaDB"),
        "{combined}"
    );
    let _ = fs::remove_dir_all(&trap);
    let _ = fs::remove_dir_all(os.parent().unwrap());
}
