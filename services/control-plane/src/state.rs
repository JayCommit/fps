use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use fps_auth::MasterKey;
use fps_config::ControlPlaneConfig;
use sqlx::MySqlPool;

use crate::ca::CertificateAuthority;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Arc<ControlPlaneConfig>,
    pub master_key: Arc<MasterKey>,
    pub ca: Arc<CertificateAuthority>,
    pub login_attempts: Arc<DashMap<String, Vec<Instant>>>,
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
