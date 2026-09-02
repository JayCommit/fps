use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use fps_auth::MasterKey;
use fps_config::ControlPlaneConfig;
use fps_domain::ServerId;
use sqlx::MySqlPool;
use tokio::sync::broadcast;

use crate::ca::CertificateAuthority;

#[derive(Clone, Debug)]
pub struct LogEvent {
    pub server_id: ServerId,
    pub stream: String,
    pub chunk: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct LogHub {
    tx: broadcast::Sender<LogEvent>,
}

impl Default for LogHub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(2048);
        Self { tx }
    }
}

impl LogHub {
    pub fn publish(&self, event: LogEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEvent> {
        self.tx.subscribe()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Arc<ControlPlaneConfig>,
    pub master_key: Arc<MasterKey>,
    pub ca: Arc<CertificateAuthority>,
    pub login_attempts: Arc<DashMap<String, Vec<Instant>>>,
    pub log_hub: Arc<LogHub>,
}

impl AppState {
    pub fn new(
        pool: MySqlPool,
        config: ControlPlaneConfig,
        master_key: MasterKey,
        ca: CertificateAuthority,
    ) -> Self {
        Self {
            pool,
            config: Arc::new(config),
            master_key: Arc::new(master_key),
            ca: Arc::new(ca),
            login_attempts: Arc::new(DashMap::new()),
            log_hub: Arc::new(LogHub::default()),
        }
    }

    /// Returns true if the caller should be rate-limited.
    pub fn record_login_failure(&self, ip: &str) -> bool {
        let mut entry = self.login_attempts.entry(ip.to_string()).or_default();
        let cutoff = Instant::now() - std::time::Duration::from_secs(60);
        entry.retain(|t| *t > cutoff);
        entry.push(Instant::now());
        entry.len() > 10
    }

    pub fn clear_login_failures(&self, ip: &str) {
        self.login_attempts.remove(ip);
    }
}
