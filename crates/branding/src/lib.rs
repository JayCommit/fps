//! Centralized branding and release identity.
//!
//! Change display names, package identifiers, GitHub coordinates, and ports
//! here. Do not scatter product strings through the rest of the codebase.

use semver::Version;
use serde::{Deserialize, Serialize};

/// Human-facing product name.
pub const DISPLAY_NAME: &str = "FPS";

/// Filesystem, package, crate, and service identifier.
pub const PACKAGE_NAME: &str = "fps";

/// Environment-variable prefix (`FPS_DATABASE_URL`, …).
pub const ENV_PREFIX: &str = "FPS";

/// systemd unit stem (`fps-control-plane.service`).
pub const UNIT_PREFIX: &str = "fps";

/// Default HTTP bind for the control-plane API (uncommon port).
pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:47890";

/// Default mTLS bind for node-agent traffic.
pub const DEFAULT_NODE_BIND: &str = "127.0.0.1:47891";

/// Default Vite / web UI bind for local development.
pub const DEFAULT_WEB_BIND: &str = "127.0.0.1:47880";

/// GitHub account that owns this repository.
pub const GITHUB_OWNER: &str = "JayCommit";

/// GitHub repository name.
pub const GITHUB_REPOSITORY: &str = "fps";

/// Product version for this workspace revision.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// User-agent used by control-plane, agent, and bootstrap HTTP clients.
pub fn user_agent() -> String {
    format!("{PACKAGE_NAME}/{VERSION}")
}

/// SemVer parsed from [`VERSION`].
pub fn version() -> Version {
    VERSION.parse().expect("workspace version is valid SemVer")
}

/// Release channel implied by the current version.
pub fn implied_channel() -> Channel {
    Channel::from_version(&version())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Alpha,
    Beta,
    Stable,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Stable => "stable",
        }
    }

    pub fn from_version(version: &Version) -> Self {
        if version.pre.is_empty() {
            return Self::Stable;
        }
        let ident = version.pre.as_str();
        if ident.starts_with("alpha") {
            Self::Alpha
        } else if ident.starts_with("beta") {
            Self::Beta
        } else {
            Self::Alpha
        }
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_alpha_1() {
        let v = version();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 1);
        assert_eq!(v.pre.as_str(), "alpha.1");
        assert_eq!(implied_channel(), Channel::Alpha);
    }

    #[test]
    fn branding_is_centralized() {
        assert_eq!(DISPLAY_NAME, "FPS");
        assert_eq!(PACKAGE_NAME, "fps");
        assert!(ENV_PREFIX
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_'));
    }
}
