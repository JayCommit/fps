use std::net::SocketAddr;

use fps_bootstrap::config::BootstrapConfig;
use fps_bootstrap::plan::build_plan;
use fps_bootstrap::preflight::run_preflight;
use fps_bootstrap::proxmox::HttpProxmox;
use fps_test_support::proxmox_fake::FakeProxmox;
use tokio::net::TcpListener;

fn sample_toml(vmid_cp: u32, vmid_node: u32) -> String {
    format!(
        r#"
schema_version = 1
product_channel = "alpha"

[control_plane]
guest_kind = "lxc"
vmid = {vmid_cp}
hostname = "fps-cp"
cores = 2
memory_mib = 4096
disk_gib = 32
storage = "local-lvm"
bridge = "vmbr0"
ip_cidr = "10.10.2.10/24"
gateway = "10.10.2.1"
dns = ["10.10.2.1"]
ssh_public_key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPlaceholderKeyDoNotUse comment"
os_template = "debian-12-standard_12.7-1_amd64.tar.zst"
os_template_sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[control_plane.proxmox]
url = "https://fry.example.test:8006"
node = "fry"
token_id = "bootstrap@pve!alpha"
token_secret_env = "FPS_FRY_TOKEN_SECRET"

[game_node]
guest_kind = "vm"
vmid = {vmid_node}
hostname = "fps-node-01"
cores = 4
memory_mib = 16384
disk_gib = 200
storage = "local-lvm"
bridge = "vmbr0"
ip_cidr = "10.10.1.20/24"
gateway = "10.10.1.1"
dns = ["10.10.1.1"]
ssh_public_key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPlaceholderKeyDoNotUse comment"
os_template = "debian-12-cloudimg-amd64.qcow2"
os_template_sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

[game_node.proxmox]
url = "https://homer.example.test:8006"
node = "homer"
token_id = "bootstrap@pve!alpha"
token_secret_env = "FPS_HOMER_TOKEN_SECRET"
"#
    )
}

#[test]
fn config_rejects_lxc_game_node() {
    let mut bad = sample_toml(120, 210);
    bad = bad.replace("guest_kind = \"vm\"", "guest_kind = \"lxc\"");
    let dir = std::env::temp_dir().join("pn-boot-bad.toml");
    std::fs::write(&dir, bad).unwrap();
    assert!(BootstrapConfig::load(&dir).is_err());
}

#[test]
fn plan_is_non_mutating_in_dry_run() {
    let path = std::env::temp_dir().join("pn-boot-ok.toml");
    std::fs::write(&path, sample_toml(120, 210)).unwrap();
    let cfg = BootstrapConfig::load(&path).unwrap();
    let plan = build_plan(&cfg, true);
    assert!(plan.actions.iter().all(|a| !a.mutating));
    assert!(plan.summary.contains("fps-cp"));
}

#[tokio::test]
async fn preflight_against_fake_proxmox() {
    let fake = FakeProxmox::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let router = fake.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let path = std::env::temp_dir().join("pn-boot-fake.toml");
    std::fs::write(&path, sample_toml(120, 210)).unwrap();
    let cfg = BootstrapConfig::load(&path).unwrap();
    let client = HttpProxmox::new(&format!("http://{addr}")).unwrap();
    let report = run_preflight(&cfg, &client, &client).await.unwrap();
    assert!(report.ok, "{report:?}");
}

#[tokio::test]
async fn preflight_against_fake_rejects_existing_vmid() {
    let fake = FakeProxmox::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let router = fake.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let path = std::env::temp_dir().join("pn-boot-taken-fake.toml");
    std::fs::write(&path, sample_toml(100, 210)).unwrap();
    let cfg = BootstrapConfig::load(&path).unwrap();
    let client = HttpProxmox::new(&format!("http://{addr}")).unwrap();
    let report = run_preflight(&cfg, &client, &client).await.unwrap();
    assert!(!report.ok, "{report:?}");
}

#[tokio::test]
async fn apply_creates_guests_on_fake_proxmox() {
    let fake = FakeProxmox::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let router = fake.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let path = std::env::temp_dir().join("pn-boot-apply.toml");
    std::fs::write(&path, sample_toml(120, 210)).unwrap();
    let cfg = BootstrapConfig::load(&path).unwrap();
    let client = HttpProxmox::new(&format!("http://{addr}")).unwrap();
    let report = run_preflight(&cfg, &client, &client).await.unwrap();
    assert!(report.ok, "{report:?}");
    let upids = fps_bootstrap::apply::apply_guests(&cfg, &client, &client)
        .await
        .unwrap();
    assert_eq!(upids.len(), 2);
    let posts = fake.recorded_posts();
    assert!(posts.iter().any(|(p, _)| p.contains("/lxc")), "{posts:?}");
    assert!(posts.iter().any(|(p, _)| p.contains("/qemu")), "{posts:?}");
}

#[tokio::test]
async fn preflight_refuses_existing_vmid_when_client_reports_in_use() {
    // Direct trait: a client that claims VMID 100 is taken.
    struct Taken;
    #[async_trait::async_trait]
    impl fps_bootstrap::proxmox::ProxmoxView for Taken {
        async fn version(&self) -> anyhow::Result<String> {
            Ok("8.3.0".into())
        }
        async fn node_online(&self, _node: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn has_storage(&self, _n: &str, _s: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn has_bridge(&self, _n: &str, _i: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn vmid_in_use(&self, vmid: u32) -> anyhow::Result<bool> {
            Ok(vmid == 100)
        }
        async fn create_lxc(&self, _n: &str, _b: serde_json::Value) -> anyhow::Result<String> {
            panic!("create_lxc must not run during preflight");
        }
        async fn create_qemu(&self, _n: &str, _b: serde_json::Value) -> anyhow::Result<String> {
            panic!("create_qemu must not run during preflight");
        }
    }
    let path = std::env::temp_dir().join("pn-boot-taken.toml");
    std::fs::write(&path, sample_toml(100, 210)).unwrap();
    let cfg = BootstrapConfig::load(&path).unwrap();
    let client = Taken;
    let report = run_preflight(&cfg, &client, &client).await.unwrap();
    assert!(!report.ok);
    assert!(report
        .checks
        .iter()
        .any(|c| !c.ok && c.name.contains("vmid")));
}
