pub mod ca;
pub mod db;
pub mod http;
pub mod mtls;
pub mod redact;
pub mod scheduler;
pub mod state;

use std::net::SocketAddr;

use anyhow::Context;
use fps_auth::MasterKey;
use fps_config::ControlPlaneConfig;
use fps_domain::Permission;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::info;

use crate::ca::CertificateAuthority;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub struct BoundAddrs {
    pub http: SocketAddr,
    pub node: SocketAddr,
}

pub async fn serve(cfg: ControlPlaneConfig) -> anyhow::Result<()> {
    serve_inner(cfg, None).await
}

pub async fn serve_ready(
    cfg: ControlPlaneConfig,
    ready: tokio::sync::oneshot::Sender<BoundAddrs>,
) -> anyhow::Result<()> {
    serve_inner(cfg, Some(ready)).await
}

async fn serve_inner(
    cfg: ControlPlaneConfig,
    ready: Option<tokio::sync::oneshot::Sender<BoundAddrs>>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&cfg.data_dir).ok();
    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;
    db::templates::ensure_catalogue(&pool).await?;
    let master_key =
        MasterKey::from_hex(&cfg.master_key_hex).map_err(|e| anyhow::anyhow!("master key: {e}"))?;
    let ca = CertificateAuthority::load_or_create(&cfg.data_dir.join("ca"))?;

    let http_listener = TcpListener::bind(cfg.http_bind)
        .await
        .with_context(|| format!("bind {}", cfg.http_bind))?;
    let node_listener = TcpListener::bind(cfg.node_bind)
        .await
        .with_context(|| format!("bind {}", cfg.node_bind))?;
    let web_listener = if let Some(bind) = cfg.web_bind {
        Some(
            TcpListener::bind(bind)
                .await
                .with_context(|| format!("bind web UI {bind}"))?,
        )
    } else {
        None
    };
    let mut cfg = cfg;
    cfg.http_bind = http_listener.local_addr()?;
    cfg.node_bind = node_listener.local_addr()?;
    if let (Some(bind), Some(listener)) = (cfg.web_bind.as_mut(), web_listener.as_ref()) {
        *bind = listener.local_addr()?;
    }
    let addrs = BoundAddrs {
        http: cfg.http_bind,
        node: cfg.node_bind,
    };
    let state = AppState::new(pool, cfg.clone(), master_key, ca);

    if let Some(root) = &cfg.web_root {
        if !root.join("index.html").is_file() {
            tracing::warn!(
                path = %root.display(),
                "FPS_WEB_ROOT has no index.html; the panel will 404 until a UI build is installed"
            );
        }
    }

    info!(
        product = fps_branding::DISPLAY_NAME,
        version = fps_branding::VERSION,
        http = %addrs.http,
        node = %addrs.node,
        web = ?cfg.web_bind,
        web_root = ?cfg.web_root,
        public_url = %cfg.public_url,
        allow_insecure_http = cfg.allow_insecure_http,
        "starting control plane"
    );

    let rustls_config = mtls::server_config(&state.ca, addrs.node, &cfg.public_url)?;
    let acceptor = TlsAcceptor::from(rustls_config);
    let public_app = http::router(state.clone());
    let node_app = http::node_router(state.clone());

    if let Some(tx) = ready {
        let _ = tx.send(addrs);
    }

    let scheduler_pool = state.pool.clone();
    let http_server = axum::serve(
        http_listener,
        public_app
            .clone()
            .into_make_service_with_connect_info::<SocketAddr>(),
    );
    let node_server = mtls::accept_loop(node_listener, acceptor, node_app);
    let scheduler = scheduler::run_loop(scheduler_pool);

    if let Some(web_listener) = web_listener {
        let web_server = axum::serve(
            web_listener,
            public_app.into_make_service_with_connect_info::<SocketAddr>(),
        );
        tokio::select! {
            result = http_server => {
                result.context("http server")?;
            }
            result = web_server => {
                result.context("web UI server")?;
            }
            result = node_server => {
                result.context("node mTLS server")?;
            }
            _ = scheduler => {}
        }
    } else {
        tokio::select! {
            result = http_server => {
                result.context("http server")?;
            }
            result = node_server => {
                result.context("node mTLS server")?;
            }
            _ = scheduler => {}
        }
    }
    Ok(())
}

pub fn dump_openapi() -> String {
    http::openapi_json()
}

pub fn dump_permissions() -> String {
    let values: Vec<&str> = Permission::ALL.iter().map(|p| p.as_str()).collect();
    serde_json::to_string_pretty(&values).expect("permissions serialize")
}
