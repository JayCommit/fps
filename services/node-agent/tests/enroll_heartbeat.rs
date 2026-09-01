use std::time::Duration;

use fps_auth::MasterKey;
use fps_config::ControlPlaneConfig;
use fps_control_plane::{serve_ready, BoundAddrs};
use fps_node_agent::{enroll, send_heartbeat, AgentConfig};

async fn spawn_plane(allow_insecure_http: bool, public_url: &str) -> (BoundAddrs, tempfile::Temp) {
    let lock = fps_test_support::lock_db().await;
    let (pool, db_name) = fps_test_support::test_pool().await.expect("MariaDB");
    fps_control_plane::db::migrate(&pool)
        .await
        .expect("migrate");
    let _ = fps_test_support::reset_schema(&pool).await;
    drop(pool);
    let dir = std::env::temp_dir().join(format!("pn-agent-e2e-{db_name}-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let (_, key) = MasterKey::generate();
    let cfg = ControlPlaneConfig {
        http_bind: "127.0.0.1:0".parse().unwrap(),
        node_bind: "127.0.0.1:0".parse().unwrap(),
        public_url: public_url.into(),
        database_url: std::env::var("FPS_TEST_DATABASE_URL")
            .unwrap_or_else(|_| "mysql://fps:local-dev-only@127.0.0.1:3306/fps_test".into()),
        master_key_hex: key,
        data_dir: dir.clone(),
        allow_insecure_http,
        session_ttl_secs: 3600,
        refresh_ttl_secs: 86400,
        enrollment_ttl_secs: 900,
        heartbeat_timeout_secs: 45,
        argon2: fps_auth::Argon2Params::for_tests(),
        cors_origins: vec!["http://127.0.0.1:47880".into()],
        cookie_secure: false,
        trust_forwarded_headers: false,
        log_format: "pretty".into(),
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if let Err(err) = serve_ready(cfg, tx).await {
            eprintln!("control plane exited: {err:#}");
        }
    });
    let addrs = tokio::time::timeout(Duration::from_secs(10), rx)
        .await
        .expect("ready timeout")
        .expect("ready");
    (
        addrs,
        tempfile::Temp {
            dir,
            _lock: lock,
            _db: db_name,
        },
    )
}

mod tempfile {
    pub struct Temp {
        pub dir: std::path::PathBuf,
        pub _lock: tokio::sync::MutexGuard<'static, ()>,
        pub _db: String,
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

async fn setup_and_token(http: std::net::SocketAddr) -> String {
    let client = reqwest::Client::new();
    let base = format!("http://{http}");
    let setup = client
        .post(format!("{base}/v1/setup"))
        .json(&serde_json::json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        setup.status().is_success(),
        "{}",
        setup.text().await.unwrap()
    );
    let body: serde_json::Value = setup.json().await.unwrap();
    let access = body["access_token"].as_str().unwrap();
    let token = client
        .post(format!("{base}/v1/nodes/enrollment-tokens"))
        .bearer_auth(access)
        .json(&serde_json::json!({ "label": "e2e" }))
        .send()
        .await
        .unwrap();
    assert!(token.status().is_success());
    let body: serde_json::Value = token.json().await.unwrap();
    body["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn agent_enrolls_and_heartbeats_over_insecure_http() {
    let (addrs, guard) = spawn_plane(true, "http://127.0.0.1:47890").await;
    let enroll_token = setup_and_token(addrs.http).await;
    let data_dir = guard.dir.join("agent");
    let cfg = AgentConfig {
        control_plane_url: format!("http://{}", addrs.http),
        data_dir,
        name: Some("e2e-node".into()),
        labels: vec!["test:e2e".into()],
        heartbeat_interval: Duration::from_secs(15),
        allow_insecure_http: true,
    };
    let identity = enroll(&cfg, &enroll_token).await.expect("enroll");
    let resp = send_heartbeat(&cfg, &identity, chrono::Utc::now())
        .await
        .expect("heartbeat");
    assert!(resp.accepted);
}

#[tokio::test]
async fn agent_heartbeats_over_mtls_when_insecure_http_is_disabled() {
    let (addrs, guard) = spawn_plane(false, "https://127.0.0.1").await;
    let enroll_token = setup_and_token(addrs.http).await;
    let data_dir = guard.dir.join("agent");
    let cfg = AgentConfig {
        control_plane_url: format!("http://{}", addrs.http),
        data_dir,
        name: Some("e2e-mtls".into()),
        labels: vec!["test:mtls".into()],
        heartbeat_interval: Duration::from_secs(15),
        allow_insecure_http: true,
    };
    let identity = enroll(&cfg, &enroll_token).await.expect("enroll");
    assert!(
        identity.node_endpoint.starts_with("https://"),
        "{}",
        identity.node_endpoint
    );
    assert!(identity
        .node_endpoint
        .contains(&addrs.node.port().to_string()));
    let resp = send_heartbeat(&cfg, &identity, chrono::Utc::now())
        .await
        .expect("mtls heartbeat");
    assert!(resp.accepted);
}
