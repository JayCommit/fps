use axum::body::Body;
use axum::http::{Request, StatusCode};
use fps_auth::MasterKey;
use fps_config::ControlPlaneConfig;
use fps_control_plane::ca::CertificateAuthority;
use fps_control_plane::http::router;
use fps_control_plane::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn app() -> (axum::Router, tempfile_guard::Guard) {
    app_with(None).await
}

async fn app_with(web_root: Option<std::path::PathBuf>) -> (axum::Router, tempfile_guard::Guard) {
    let lock = fps_test_support::lock_db().await;
    let (pool, db_name) = fps_test_support::test_pool()
        .await
        .expect("MariaDB must be reachable for integration tests (see .env.test.example)");
    fps_control_plane::db::migrate(&pool)
        .await
        .expect("migrate");
    let _ = fps_test_support::reset_schema(&pool).await;
    let dir = std::env::temp_dir().join(format!("pn-ca-{db_name}"));
    let _ = std::fs::create_dir_all(&dir);
    let ca = CertificateAuthority::load_or_create(&dir).expect("ca");
    let (_, key) = MasterKey::generate();
    let cfg = ControlPlaneConfig {
        http_bind: "127.0.0.1:0".parse().unwrap(),
        node_bind: "127.0.0.1:0".parse().unwrap(),
        public_url: "http://127.0.0.1:47890".into(),
        database_url: "mysql://unused".into(),
        master_key_hex: key,
        data_dir: dir.clone(),
        allow_insecure_http: true,
        session_ttl_secs: 3600,
        refresh_ttl_secs: 86400,
        enrollment_ttl_secs: 900,
        heartbeat_timeout_secs: 45,
        argon2: fps_auth::Argon2Params::for_tests(),
        cors_origins: vec!["http://127.0.0.1:47880".into()],
        cookie_secure: false,
        trust_forwarded_headers: false,
        log_format: "pretty".into(),
        web_root,
        web_bind: None,
    };
    let master = MasterKey::from_hex(&cfg.master_key_hex).unwrap();
    let state = AppState::new(pool, cfg, master, ca);
    (
        router(state),
        tempfile_guard::Guard {
            db_name,
            dir,
            _lock: lock,
        },
    )
}

mod tempfile_guard {
    pub struct Guard {
        pub db_name: String,
        pub dir: std::path::PathBuf,
        pub _lock: tokio::sync::MutexGuard<'static, ()>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
            let _ = &self.db_name;
        }
    }
}

async fn json(
    app: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    builder = builder.header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let body = body
        .map(|v| Body::from(serde_json::to_vec(&v).unwrap()))
        .unwrap_or_else(Body::empty);
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, value)
}

#[tokio::test]
async fn vertical_slice_setup_enroll_heartbeat() {
    let (app, _guard) = app().await;

    let (status, body) = json(&app, "GET", "/health", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = json(&app, "GET", "/v1/setup/status", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["completed"], false);

    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap().to_string();
    assert!(!access.is_empty());

    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "other@example.test",
            "password": "correct horse battery",
            "display_name": "Other"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, body) = json(&app, "GET", "/v1/auth/me", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["role"], "owner");

    let (status, _) = json(
        &app,
        "GET",
        &format!("/v1/auth/me?access_token={access}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let ws_query = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/auth/me?access_token={access}"))
                .header("upgrade", "websocket")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ws_query.status(), StatusCode::OK);

    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enrollment-tokens",
        Some(&access),
        Some(json!({ "label": "local-test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let enroll_token = body["token"].as_str().unwrap().to_string();

    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enroll",
        None,
        Some(json!({
            "enrollment_token": enroll_token,
            "hostname": "homer-node-01",
            "name": "Homer",
            "agent_version": "0.0.1-alpha.1",
            "protocol_version": 1,
            "architecture": "x86_64",
            "operating_system": "linux",
            "labels": ["site:homer"],
            "docker": { "state": "unavailable" },
            "resources": { "cpu_cores": 8, "memory_bytes": 32000000000u64 }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let node_id = body["node_id"].as_str().unwrap().to_string();
    let node_token = body["node_token"].as_str().unwrap().to_string();
    assert!(body["certificate_pem"]
        .as_str()
        .unwrap()
        .contains("BEGIN CERTIFICATE"));

    // Replay must fail.
    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enroll",
        None,
        Some(json!({
            "enrollment_token": enroll_token,
            "hostname": "replay",
            "agent_version": "0.0.1-alpha.1",
            "protocol_version": 1,
            "architecture": "x86_64",
            "operating_system": "linux",
            "labels": [],
            "docker": { "state": "unavailable" },
            "resources": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "unavailable", "error": "no docker socket" },
            "resources": { "cpu_cores": 8 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["accepted"], true);
    assert_eq!(body["settings"]["uninstall"], false);
    assert_eq!(body["settings"]["heartbeat_interval_seconds"], 15);

    let (status, body) = json(&app, "GET", "/v1/nodes", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[0]["health"]["status"], "online");
    assert_eq!(body[0]["hostname"], "homer-node-01");

    let (status, body) = json(&app, "GET", "/v1/dashboard", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["nodes_online"], 1);

    let (status, body) = json(&app, "GET", "/version", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["database_schema"], 6);

    let (status, _) = json(&app, "GET", "/v1/auth/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_rejects_bad_password() {
    let (app, _guard) = app().await;
    let _ = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    let (status, _) = json(
        &app,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "definitely wrong!!"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn viewer_cannot_enroll_without_permission_check_on_missing_auth() {
    let (app, _guard) = app().await;
    let (status, _) = json(&app, "GET", "/v1/nodes", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn cookie_session_is_rejected() {
    let (app, _guard) = app().await;
    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/v1/auth/me")
        .header("cookie", format!("session={access}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_rotates_tokens_and_rejects_unknown_refresh() {
    let (app, _guard) = app().await;
    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    let old_access = body["access_token"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_ne!(body["access_token"].as_str().unwrap(), old_access);
    assert!(!body["csrf_token"].as_str().unwrap().is_empty());
    let (status, _) = json(
        &app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn totp_start_does_not_disable_enabled_mfa() {
    let (app, _guard) = app().await;
    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap().to_string();
    let (status, body) = json(&app, "POST", "/v1/auth/totp/start", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let otpauth = body["otpauth_url"].as_str().unwrap();
    let totp = totp_rs::TOTP::from_url(otpauth).expect("otpauth");
    let code = totp.generate_current().unwrap();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/auth/totp/confirm",
        Some(&access),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let recovery = body["recovery_codes"][0].as_str().unwrap().to_string();

    let (status, _) = json(&app, "POST", "/v1/auth/totp/start", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json(
        &app,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(body["type"].as_str().unwrap().contains("mfa_required"));

    let (status, body) = json(
        &app,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "recovery_code": recovery
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body["access_token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn viewer_cannot_issue_enrollment_tokens() {
    let (app, _guard) = app().await;
    let (status, _) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (pool, _) = fps_test_support::test_pool().await.expect("test db");
    let hash =
        fps_auth::hash_password("viewer password!!", fps_auth::Argon2Params::for_tests()).unwrap();
    fps_control_plane::db::users::insert_user(
        &pool,
        fps_domain::UserId::new(),
        "viewer@example.test",
        "Viewer",
        fps_domain::Role::Viewer,
        &hash,
    )
    .await
    .expect("insert viewer");

    let (status, body) = json(
        &app,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({
            "email": "viewer@example.test",
            "password": "viewer password!!"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap().to_string();
    let (status, body) = json(&app, "GET", "/v1/nodes", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enrollment-tokens",
        Some(&access),
        Some(json!({ "label": "nope" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn metrics_and_docs_require_auth() {
    let (app, _guard) = app().await;
    let (status, _) = json(&app, "GET", "/metrics", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = json(&app, "GET", "/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = json(&app, "GET", "/docs", None, None).await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "{status}"
    );

    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap().to_string();
    let (status, _) = json(&app, "GET", "/metrics", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = json(&app, "GET", "/openapi.json", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["openapi"].as_str().unwrap().starts_with("3."));
}

#[tokio::test]
async fn invitation_accept_creates_operator() {
    let (app, _guard) = app().await;
    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/invitations",
        Some(&access),
        Some(json!({ "email": "op@example.test", "role": "operator" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["token"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/invitations/accept",
        None,
        Some(json!({
            "token": token,
            "password": "operator pass!!",
            "display_name": "Operator"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["role"], "operator");
    let (status, body) = json(&app, "GET", "/v1/users", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u["email"] == "op@example.test"));
}

#[tokio::test]
async fn template_catalogue_and_server_install_job() {
    let (app, _guard) = app().await;
    let (status, body) = json(
        &app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = body["access_token"].as_str().unwrap().to_string();

    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enrollment-tokens",
        Some(&access),
        Some(json!({ "label": "job-test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let enroll_token = body["token"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enroll",
        None,
        Some(json!({
            "enrollment_token": enroll_token,
            "hostname": "homer-docker",
            "name": "Homer",
            "agent_version": "0.0.1-alpha.1",
            "protocol_version": 1,
            "architecture": "x86_64",
            "operating_system": "linux",
            "labels": [],
            "docker": { "state": "available", "engine_version": "27.0" },
            "resources": { "cpu_cores": 4, "memory_bytes": 8000000000u64 }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let node_id = body["node_id"].as_str().unwrap().to_string();
    let node_token = body["node_token"].as_str().unwrap().to_string();

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = json(&app, "GET", "/v1/templates", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let echo = body
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["slug"] == "http-echo")
        .expect("http-echo catalogue");
    let template_id = echo["id"].as_str().unwrap();

    let (status, body) = json(
        &app,
        "POST",
        "/v1/servers",
        Some(&access),
        Some(json!({
            "name": "echo-1",
            "template_id": template_id
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let server_id = body["id"].as_str().unwrap().to_string();
    assert_eq!(body["status"], "installing");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0,
            "container_samples": [{ "server_id": server_id, "running": false }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["jobs"][0]["kind"], "install");
    assert_eq!(body["jobs"].as_array().unwrap().len(), 1);
    let job_id = body["jobs"][0]["id"].as_str().unwrap().to_string();

    let (status, body) = json(
        &app,
        "GET",
        &format!("/v1/servers/{server_id}"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "installing");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 1,
            "job_results": [{
                "id": job_id,
                "success": true,
                "message": "started",
                "container_id": "abc123",
                "container_name": "pn-echo"
            }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = json(
        &app,
        "GET",
        &format!("/v1/servers/{server_id}"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "running");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/revoke"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": {},
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::UNAUTHORIZED,
        "{status} {body}"
    );
}

#[tokio::test]
async fn root_is_not_found_without_web_root() {
    let (app, _guard) = app().await;
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn web_root_serves_spa_and_does_not_mask_api_404() {
    let web = std::env::temp_dir().join(format!(
        "fps-web-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(
        web.join("index.html"),
        b"<!doctype html><title>FPS panel</title>",
    )
    .unwrap();
    std::fs::write(web.join("assets/app.js"), b"window.FPS=1").unwrap();
    let (app, _guard) = app_with(Some(web.clone())).await;

    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("FPS panel"));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/servers/abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("FPS panel"));

    let (status, _) = json(&app, "GET", "/v1/does-not-exist", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(&web);
}

async fn owner_session(app: &axum::Router) -> String {
    let (status, body) = json(
        app,
        "POST",
        "/v1/setup",
        None,
        Some(json!({
            "email": "owner@example.test",
            "password": "correct horse battery",
            "display_name": "Owner"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn settings_and_update_check_are_authenticated() {
    let (app, _guard) = app().await;
    let (status, _) = json(&app, "GET", "/v1/settings", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let access = owner_session(&app).await;
    let (status, body) = json(&app, "GET", "/v1/settings", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["version"], "0.0.1-alpha.1");
    assert_eq!(body["public_url"], "http://127.0.0.1:47890");

    let (status, body) = json(
        &app,
        "PATCH",
        "/v1/settings",
        Some(&access),
        Some(json!({ "operator_notes": "lab notes" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["operator_notes"], "lab notes");

    let (status, body) = json(&app, "GET", "/v1/updates/check", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["current_version"], "0.0.1-alpha.1");
    assert!(
        body["releases_url"].as_str().unwrap().contains("/releases"),
        "{body}"
    );
    assert!(
        !body["releases_url"]
            .as_str()
            .unwrap()
            .contains("/releases/latest"),
        "{body}"
    );
}

#[tokio::test]
async fn restore_rejects_unknown_and_unfinished_backups() {
    let (app, _guard) = app().await;
    let access = owner_session(&app).await;
    let (status, body) = json(
        &app,
        "POST",
        "/v1/backups/not-a-uuid/restore",
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let missing = fps_domain::BackupId::new();
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/backups/{missing}/restore"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

fn heartbeat_payload() -> Value {
    json!({
        "protocol_version": 1,
        "agent_version": "0.0.1-alpha.1",
        "docker": { "state": "available", "engine_version": "27.0" },
        "resources": {
            "cpu_cores": 8,
            "memory_bytes": 16000000000u64,
            "memory_used_bytes": 8000000000u64,
            "disk_bytes": 500000000000u64,
            "disk_available_bytes": 180000000000u64,
            "load_one": 1.25,
            "cpu_percent": 33.5,
            "uptime_seconds": 7200
        },
        "started_at": chrono::Utc::now(),
        "workload_count": 0
    })
}

async fn enroll_lab_node(app: &axum::Router, access: &str) -> (String, String) {
    let (status, body) = json(
        app,
        "POST",
        "/v1/nodes/enrollment-tokens",
        Some(access),
        Some(json!({ "label": "ops-test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let enroll_token = body["token"].as_str().unwrap().to_string();
    let (status, body) = json(
        app,
        "POST",
        "/v1/nodes/enroll",
        None,
        Some(json!({
            "enrollment_token": enroll_token,
            "hostname": "lab-host-01",
            "name": "lab-host-01",
            "agent_version": "0.0.1-alpha.1",
            "protocol_version": 1,
            "architecture": "x86_64",
            "operating_system": "linux",
            "labels": ["site:lab"],
            "docker": { "state": "available", "engine_version": "27.0" },
            "resources": {
                "cpu_cores": 8,
                "memory_bytes": 16000000000u64,
                "memory_used_bytes": 4000000000u64
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["node_id"].as_str().unwrap().to_string(),
        body["node_token"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn panel_manages_host_settings_prune_and_uninstall() {
    let (app, _guard) = app().await;
    let access = owner_session(&app).await;
    let (node_id, node_token) = enroll_lab_node(&app, &access).await;

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(heartbeat_payload()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["name"], "lab-host-01");
    assert_eq!(body["settings"]["heartbeat_interval_seconds"], 15);
    assert_eq!(body["settings"]["uninstall"], false);
    assert_eq!(body["desired_drain"], false);

    let (status, body) = json(&app, "GET", &format!("/v1/nodes/{node_id}"), Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["health"]["resources"]["cpu_percent"], 33.5);
    assert_eq!(body["health"]["resources"]["memory_used_bytes"], 8000000000u64);
    assert_eq!(body["health"]["resources"]["uptime_seconds"], 7200);
    assert_eq!(body["heartbeat_interval_seconds"], 15);

    let (status, body) = json(
        &app,
        "PATCH",
        &format!("/v1/nodes/{node_id}"),
        Some(&access),
        Some(json!({ "heartbeat_interval_seconds": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = json(
        &app,
        "PATCH",
        &format!("/v1/nodes/{node_id}"),
        Some(&access),
        Some(json!({
            "name": "edge-lab",
            "labels": ["site:lab", "role:game"],
            "maintenance": true,
            "heartbeat_interval_seconds": 30
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "edge-lab");
    assert_eq!(body["maintenance"], true);
    assert_eq!(body["heartbeat_interval_seconds"], 30);
    assert_eq!(body["health"]["status"], "maintenance");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(heartbeat_payload()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["name"], "edge-lab");
    assert_eq!(body["settings"]["heartbeat_interval_seconds"], 30);
    assert_eq!(body["settings"]["maintenance"], true);
    assert_eq!(body["desired_drain"], true);
    assert_eq!(body["jobs"].as_array().unwrap().len(), 0);

    let (status, body) = json(&app, "GET", "/v1/templates", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let echo = body
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["slug"] == "http-echo")
        .expect("http-echo catalogue");
    let template_id = echo["id"].as_str().unwrap();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/servers",
        Some(&access),
        Some(json!({ "name": "should-not-schedule", "template_id": template_id })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/docker-prune"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(heartbeat_payload()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["docker_prune"], true);

    let mut ack_prune = heartbeat_payload();
    ack_prune["control_ack"] = json!({ "docker_prune": "completed" });
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(ack_prune),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["docker_prune"], false);

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/uninstall"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["uninstall_requested"], true);

    let (status, body) = json(&app, "GET", &format!("/v1/nodes/{node_id}"), Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["uninstall_requested"], true);
    assert_eq!(body["maintenance"], true);

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(heartbeat_payload()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["uninstall"], true);
    assert_eq!(body["desired_drain"], true);
    assert_eq!(body["jobs"].as_array().unwrap().len(), 0);

    let mut ack_uninstall = heartbeat_payload();
    ack_uninstall["control_ack"] = json!({ "uninstall": "completed" });
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(ack_uninstall),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = json(&app, "GET", &format!("/v1/nodes/{node_id}"), Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["revoked"], true);
    assert!(body["uninstalled_at"].as_str().is_some());

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(heartbeat_payload()),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::UNAUTHORIZED,
        "{status} {body}"
    );

    let (status, body) = json(
        &app,
        "PATCH",
        &format!("/v1/nodes/{node_id}"),
        Some(&access),
        Some(json!({ "name": "nope" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/uninstall"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["already_revoked"], true);
}


#[tokio::test]
async fn addons_install_uninstall_for_cs2() {
    let (app, _guard) = app().await;
    let access = owner_session(&app).await;

    let (status, _) = json(&app, "GET", "/v1/addons", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = json(&app, "GET", "/v1/addons?game=cs2", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let slugs: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["slug"].as_str().unwrap())
        .collect();
    assert!(slugs.contains(&"cs2-metamod"));
    assert!(slugs.contains(&"cs2-counterstrikesharp"));
    assert!(!slugs.iter().any(|s| s.starts_with("mc-")));

    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enrollment-tokens",
        Some(&access),
        Some(json!({ "label": "addon-node" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let enroll_token = body["token"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "POST",
        "/v1/nodes/enroll",
        None,
        Some(json!({
            "enrollment_token": enroll_token,
            "hostname": "addon-host",
            "agent_version": "0.0.1-alpha.1",
            "protocol_version": 1,
            "architecture": "x86_64",
            "operating_system": "linux",
            "labels": [],
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let node_id = body["node_id"].as_str().unwrap().to_string();
    let node_token = body["node_token"].as_str().unwrap().to_string();
    let (status, _) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json(&app, "GET", "/v1/templates", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let echo_id = body
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["slug"] == "http-echo")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let cs2_id = body
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["slug"] == "cs2")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, body) = json(
        &app,
        "POST",
        "/v1/servers",
        Some(&access),
        Some(json!({ "name": "echo-addons", "template_id": echo_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let echo_server = body["id"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "GET",
        &format!("/v1/servers/{echo_server}/addons"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.as_array().unwrap().is_empty());
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/servers/{echo_server}/addons/cs2-metamod/install"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = json(
        &app,
        "POST",
        "/v1/servers",
        Some(&access),
        Some(json!({ "name": "cs2-1", "template_id": cs2_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let server_id = body["id"].as_str().unwrap().to_string();

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let install_results: Vec<Value> = body["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| {
            json!({
                "id": j["id"],
                "success": true,
                "message": "installed",
                "container_name": "fps-test"
            })
        })
        .collect();
    let (status, _) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 2,
            "job_results": install_results
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json(
        &app,
        "GET",
        &format!("/v1/servers/{server_id}/addons"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["slug"] == "cs2-metamod" && a["status"] == "available"));

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/servers/{server_id}/addons/cs2-counterstrikesharp/install"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["kind"], "addon_install");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let jobs = body["jobs"].as_array().unwrap();
    let addon_jobs: Vec<&Value> = jobs
        .iter()
        .filter(|j| j["kind"] == "addon_install")
        .collect();
    assert!(addon_jobs.len() >= 2, "{body}");
    let results: Vec<Value> = addon_jobs
        .iter()
        .map(|j| {
            json!({
                "id": j["id"],
                "success": true,
                "message": "installed",
                "tracked_paths": ["game/csgo/addons/metamod"]
            })
        })
        .collect();

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": { "cpu_cores": 4 },
            "started_at": chrono::Utc::now(),
            "workload_count": 0,
            "job_results": results
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = json(
        &app,
        "GET",
        &format!("/v1/servers/{server_id}/addons"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let metamod = body
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["slug"] == "cs2-metamod")
        .unwrap();
    let css = body
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["slug"] == "cs2-counterstrikesharp")
        .unwrap();
    assert_eq!(metamod["status"], "installed", "{body}");
    assert_eq!(css["status"], "installed", "{body}");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/servers/{server_id}/addons/cs2-metamod/uninstall"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/servers/{server_id}/addons/cs2-counterstrikesharp/uninstall"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let uninstall_id = body["id"].as_str().unwrap().to_string();
    let (status, body) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": {},
            "started_at": chrono::Utc::now(),
            "workload_count": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["jobs"][0]["id"], uninstall_id);
    let (status, _) = json(
        &app,
        "POST",
        &format!("/v1/nodes/{node_id}/heartbeat"),
        Some(&node_token),
        Some(json!({
            "protocol_version": 1,
            "agent_version": "0.0.1-alpha.1",
            "docker": { "state": "available" },
            "resources": {},
            "started_at": chrono::Utc::now(),
            "workload_count": 0,
            "job_results": [{
                "id": uninstall_id,
                "success": true,
                "message": "uninstalled"
            }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json(
        &app,
        "GET",
        &format!("/v1/servers/{server_id}/addons"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let css = body
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["slug"] == "cs2-counterstrikesharp")
        .unwrap();
    assert_eq!(css["status"], "available", "{body}");
}
