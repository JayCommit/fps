use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fps_auth::Argon2Params;
use fps_branding::{DISPLAY_NAME, ENV_PREFIX, PACKAGE_NAME};
use fps_domain::{ErrorCode, PlatformError};
use fps_observability::looks_secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneConfig {
    pub http_bind: SocketAddr,
    pub node_bind: SocketAddr,
    pub public_url: String,
    pub database_url: String,
    pub master_key_hex: String,
    pub data_dir: PathBuf,
    pub allow_insecure_http: bool,
    pub session_ttl_secs: u64,
    pub refresh_ttl_secs: u64,
    pub enrollment_ttl_secs: u64,
    pub heartbeat_timeout_secs: i64,
    pub argon2: Argon2Params,
    pub cors_origins: Vec<String>,
    pub cookie_secure: bool,
    pub trust_forwarded_headers: bool,
    pub log_format: String,
    /// Directory of a production web UI build (`index.html` + assets).
    /// When set, the HTTP API also serves the panel (same origin as `/v1`).
    pub web_root: Option<PathBuf>,
    /// Optional second HTTP bind that serves the same router as `http_bind`
    /// (panel on :47880, API on :47890).
    pub web_bind: Option<SocketAddr>,
}

impl ControlPlaneConfig {
    pub fn from_env() -> Result<Self, PlatformError> {
        let http_bind = parse_addr(&env_or("HTTP_BIND", fps_branding::DEFAULT_HTTP_BIND))?;
        let node_bind = parse_addr(&env_or("NODE_BIND", fps_branding::DEFAULT_NODE_BIND))?;
        let public_url = env_or("PUBLIC_URL", &format!("http://{http_bind}"));
        let database_url = require_env("DATABASE_URL")?;
        let master_key_hex = require_env("MASTER_KEY")?;
        let data_dir = PathBuf::from(env_or("DATA_DIR", "./data"));
        let allow_insecure_http = env_bool("ALLOW_INSECURE_HTTP", false);
        let argon2 = Argon2Params {
            memory_kib: env_u32("ARGON2_MEMORY_KIB", Argon2Params::default().memory_kib),
            iterations: env_u32("ARGON2_ITERATIONS", Argon2Params::default().iterations),
            parallelism: env_u32("ARGON2_PARALLELISM", Argon2Params::default().parallelism),
        };
        let cors_origins = env::var(format!("{ENV_PREFIX}_CORS_ORIGINS"))
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| vec![public_url.clone(), "http://127.0.0.1:47880".into()]);
        let web_root = env::var(format!("{ENV_PREFIX}_WEB_ROOT"))
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        let web_bind = match env::var(format!("{ENV_PREFIX}_WEB_BIND")) {
            Ok(v) if v.trim().is_empty() => None,
            Ok(v) => Some(parse_addr(v.trim())?),
            Err(_) => None,
        };

        let cfg = Self {
            http_bind,
            node_bind,
            public_url,
            database_url,
            master_key_hex,
            data_dir,
            allow_insecure_http,
            session_ttl_secs: env_u64("SESSION_TTL_SECS", 12 * 3600),
            refresh_ttl_secs: env_u64("REFRESH_TTL_SECS", 14 * 24 * 3600),
            enrollment_ttl_secs: env_u64("ENROLLMENT_TTL_SECS", 15 * 60),
            heartbeat_timeout_secs: env_u64("HEARTBEAT_TIMEOUT_SECS", 45) as i64,
            argon2,
            cors_origins,
            cookie_secure: env_bool("COOKIE_SECURE", false),
            trust_forwarded_headers: env_bool("TRUST_FORWARDED_HEADERS", false),
            log_format: env_or("LOG_FORMAT", "pretty"),
            web_root,
            web_bind,
        };
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), PlatformError> {
        if self.master_key_hex.len() != 64
            || !self.master_key_hex.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(PlatformError::new(
                ErrorCode::InsecureConfiguration,
                "MASTER_KEY must be 32 bytes encoded as 64 hex characters.",
            ));
        }
        if !self.allow_insecure_http && self.public_url.starts_with("http://") {
            return Err(PlatformError::new(
                ErrorCode::InsecureConfiguration,
                "Public URL must be https:// unless ALLOW_INSECURE_HTTP is set for local development.",
            ));
        }
        if self.argon2.memory_kib < 8_192 {
            return Err(PlatformError::new(
                ErrorCode::InsecureConfiguration,
                "Argon2 memory must be at least 8192 KiB.",
            ));
        }
        if let Some(web_bind) = self.web_bind {
            if web_bind == self.http_bind {
                return Err(PlatformError::new(
                    ErrorCode::InsecureConfiguration,
                    "WEB_BIND must differ from HTTP_BIND.",
                ));
            }
        }
        Ok(())
    }

    pub fn session_ttl(&self) -> Duration {
        Duration::from_secs(self.session_ttl_secs)
    }

    pub fn redacted_diagnostics(&self) -> serde_json::Value {
        serde_json::json!({
            "product": DISPLAY_NAME,
            "package": PACKAGE_NAME,
            "http_bind": self.http_bind.to_string(),
            "node_bind": self.node_bind.to_string(),
            "public_url": self.public_url,
            "database_url": redact_url(&self.database_url),
            "data_dir": self.data_dir,
            "allow_insecure_http": self.allow_insecure_http,
            "trust_forwarded_headers": self.trust_forwarded_headers,
            "cors_origins": self.cors_origins,
            "argon2": self.argon2,
            "heartbeat_timeout_secs": self.heartbeat_timeout_secs,
            "web_root": self.web_root,
            "web_bind": self.web_bind.map(|a| a.to_string()),
        })
    }

    pub fn load_optional_file(path: &Path) -> Result<Option<toml::Value>, PlatformError> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(path)
            .map_err(|e| PlatformError::new(ErrorCode::InsecureConfiguration, e.to_string()))?;
        toml::from_str(&raw)
            .map(Some)
            .map_err(|e| PlatformError::new(ErrorCode::InsecureConfiguration, e.to_string()))
    }
}

fn env_or(suffix: &str, default: &str) -> String {
    env::var(format!("{ENV_PREFIX}_{suffix}")).unwrap_or_else(|_| default.to_string())
}

fn require_env(suffix: &str) -> Result<String, PlatformError> {
    env::var(format!("{ENV_PREFIX}_{suffix}")).map_err(|_| {
        PlatformError::new(
            ErrorCode::InsecureConfiguration,
            format!("missing required environment variable {ENV_PREFIX}_{suffix}"),
        )
    })
}

fn env_bool(suffix: &str, default: bool) -> bool {
    env::var(format!("{ENV_PREFIX}_{suffix}"))
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(default)
}

fn env_u32(suffix: &str, default: u32) -> u32 {
    env::var(format!("{ENV_PREFIX}_{suffix}"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(suffix: &str, default: u64) -> u64 {
    env::var(format!("{ENV_PREFIX}_{suffix}"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_addr(value: &str) -> Result<SocketAddr, PlatformError> {
    value.parse().map_err(|_| {
        PlatformError::new(
            ErrorCode::InsecureConfiguration,
            format!("invalid socket address '{value}'"),
        )
    })
}

pub fn redact_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            if u.password().is_some() {
                let _ = u.set_password(Some("***"));
            }
            u.to_string()
        }
        Err(_) => {
            if looks_secret(url) {
                "[redacted]".into()
            } else {
                url.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_url_password_is_redacted() {
        let redacted = redact_url("mysql://user:super-secret@127.0.0.1:3306/db");
        assert!(!redacted.contains("super-secret"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn web_bind_must_differ_from_http_bind() {
        let http: std::net::SocketAddr = "127.0.0.1:47890".parse().unwrap();
        let cfg = ControlPlaneConfig {
            http_bind: http,
            node_bind: "127.0.0.1:47891".parse().unwrap(),
            public_url: "https://panel.example".into(),
            database_url: "mysql://fps@127.0.0.1/fps".into(),
            master_key_hex: "ab".repeat(32),
            data_dir: ".".into(),
            allow_insecure_http: false,
            session_ttl_secs: 3600,
            refresh_ttl_secs: 86400,
            enrollment_ttl_secs: 900,
            heartbeat_timeout_secs: 45,
            argon2: fps_auth::Argon2Params::default(),
            cors_origins: vec![],
            cookie_secure: true,
            trust_forwarded_headers: false,
            log_format: "json".into(),
            web_root: None,
            web_bind: Some(http),
        };
        assert!(cfg.validate().is_err());
    }
}
