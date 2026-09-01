//! Which side of the Fry / Homer split this command is for.

use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::str::FromStr;

use anyhow::{bail, Result};
use serde::Serialize;

/// What to install or provision.
///
/// Fry runs the control plane (web panel + API). Homer runs the game host
/// (Docker + node agent). `Both` is for a single lab machine only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallRole {
    ControlPlane,
    GameHost,
    #[default]
    Both,
}

impl InstallRole {
    pub fn includes_control_plane(self) -> bool {
        matches!(self, Self::ControlPlane | Self::Both)
    }

    pub fn includes_game_host(self) -> bool {
        matches!(self, Self::GameHost | Self::Both)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ControlPlane => "control-plane",
            Self::GameHost => "game-host",
            Self::Both => "both",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::ControlPlane => "control plane (web panel + API)",
            Self::GameHost => "game host (Docker + node agent)",
            Self::Both => "control plane and game host on one machine",
        }
    }

    /// Parse a menu digit or a role name (`web`, `fry`, `homer`, …).
    pub fn parse_choice(raw: &str) -> Result<Self> {
        raw.parse()
    }

    /// Ask on stdin when it is a TTY. Non-interactive sessions must pass `--role`.
    pub fn prompt_or_require_flag(explicit: Option<Self>) -> Result<Self> {
        if let Some(role) = explicit {
            return Ok(role);
        }
        if !io::stdin().is_terminal() {
            bail!("no TTY: pass --role control-plane, --role game-host, or --role both");
        }
        Self::prompt()
    }

    pub fn prompt() -> Result<Self> {
        let mut stdout = io::stdout();
        writeln!(stdout, "\nFPS installer — what should this machine be?\n")?;
        writeln!(
            stdout,
            "  1) Control plane   web panel + API          (Fry)"
        )?;
        writeln!(
            stdout,
            "  2) Game host       Docker + node agent      (Homer)"
        )?;
        writeln!(
            stdout,
            "  3) Both            lab only, not the usual two-host split\n"
        )?;
        write!(stdout, "Select 1, 2, or 3: ")?;
        stdout.flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        Self::parse_choice(line.trim())
    }
}

impl fmt::Display for InstallRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InstallRole {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1" | "control-plane" | "control_plane" | "controlplane" | "web" | "panel" | "api"
            | "fry" => Ok(Self::ControlPlane),
            "2" | "game-host" | "game_host" | "gamehost" | "node" | "agent" | "homer" => {
                Ok(Self::GameHost)
            }
            "3" | "both" | "all" | "lab" => Ok(Self::Both),
            other => {
                bail!("unknown role '{other}'. Use control-plane, game-host, or both (or 1/2/3).")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_map_to_roles() {
        assert_eq!(
            InstallRole::parse_choice("1").unwrap(),
            InstallRole::ControlPlane
        );
        assert_eq!(
            InstallRole::parse_choice("web").unwrap(),
            InstallRole::ControlPlane
        );
        assert_eq!(
            InstallRole::parse_choice("Fry").unwrap(),
            InstallRole::ControlPlane
        );
        assert_eq!(
            InstallRole::parse_choice("2").unwrap(),
            InstallRole::GameHost
        );
        assert_eq!(
            InstallRole::parse_choice("homer").unwrap(),
            InstallRole::GameHost
        );
        assert_eq!(
            InstallRole::parse_choice("node").unwrap(),
            InstallRole::GameHost
        );
        assert_eq!(InstallRole::parse_choice("3").unwrap(), InstallRole::Both);
        assert!(InstallRole::parse_choice("wings").is_err());
    }

    #[test]
    fn both_includes_each_side() {
        assert!(InstallRole::Both.includes_control_plane());
        assert!(InstallRole::Both.includes_game_host());
        assert!(!InstallRole::ControlPlane.includes_game_host());
        assert!(!InstallRole::GameHost.includes_control_plane());
    }
}
