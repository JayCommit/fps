use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub node_token: String,
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub ca_pem: String,
    pub control_plane_url: String,
    #[serde(default)]
    pub node_endpoint: String,
    pub heartbeat_interval_seconds: u64,
}

impl NodeIdentity {
    pub fn heartbeat_base_url(&self) -> &str {
        if self.node_endpoint.is_empty() {
            &self.control_plane_url
        } else {
            &self.node_endpoint
        }
    }

    /// Enroll already required `--allow-insecure-http` to store an `http://` endpoint.
    /// `run` must honor that stored URL; systemd does not pass the flag.
    pub fn allows_insecure_http(&self) -> bool {
        self.heartbeat_base_url().starts_with("http://")
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        fs::create_dir_all(data_dir)?;
        let path = data_dir.join("identity.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }
        fs::write(data_dir.join("node.crt"), &self.certificate_pem)?;
        fs::write(data_dir.join("node.key"), &self.private_key_pem)?;
        fs::write(data_dir.join("ca.crt"), &self.ca_pem)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(data_dir.join("node.key"))?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(data_dir.join("node.key"), perms)?;
        }
        Ok(())
    }

    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("identity.json");
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("missing identity at {}", path.display()))?;
        Ok(serde_json::from_str(&raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(endpoint: &str, url: &str) -> NodeIdentity {
        NodeIdentity {
            node_id: "n".into(),
            node_token: "t".into(),
            certificate_pem: String::new(),
            private_key_pem: String::new(),
            ca_pem: String::new(),
            control_plane_url: url.into(),
            node_endpoint: endpoint.into(),
            heartbeat_interval_seconds: 15,
        }
    }

    #[test]
    fn http_node_endpoint_opts_in_to_insecure_heartbeats() {
        let id = sample("http://10.0.0.8:47890", "http://10.0.0.8:47890");
        assert!(id.allows_insecure_http());
        assert_eq!(id.heartbeat_base_url(), "http://10.0.0.8:47890");
    }

    #[test]
    fn mtls_endpoint_does_not_opt_in_even_if_enroll_url_was_http() {
        let id = sample("https://10.0.0.8:47891", "http://10.0.0.8:47890");
        assert!(!id.allows_insecure_http());
        assert_eq!(id.heartbeat_base_url(), "https://10.0.0.8:47891");
    }

    #[test]
    fn empty_endpoint_falls_back_to_control_plane_url() {
        let id = sample("", "http://10.0.0.8:47890");
        assert!(id.allows_insecure_http());
        assert_eq!(id.heartbeat_base_url(), "http://10.0.0.8:47890");
    }
}
