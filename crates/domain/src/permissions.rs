use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Role;

/// Canonical permission identifiers. Backend enforcement and frontend capability
/// display must use these exact strings. The frontend is never the security boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum Permission {
    #[serde(rename = "platform.setup")]
    PlatformSetup,
    #[serde(rename = "platform.settings.read")]
    PlatformSettingsRead,
    #[serde(rename = "platform.settings.write")]
    PlatformSettingsWrite,
    #[serde(rename = "identity.users.read")]
    IdentityUsersRead,
    #[serde(rename = "identity.users.write")]
    IdentityUsersWrite,
    #[serde(rename = "identity.sessions.revoke")]
    IdentitySessionsRevoke,
    #[serde(rename = "nodes.read")]
    NodesRead,
    #[serde(rename = "nodes.enroll")]
    NodesEnroll,
    #[serde(rename = "nodes.write")]
    NodesWrite,
    #[serde(rename = "audit.read")]
    AuditRead,
    #[serde(rename = "diagnostics.read")]
    DiagnosticsRead,
    #[serde(rename = "servers.read")]
    ServersRead,
    #[serde(rename = "servers.write")]
    ServersWrite,
    #[serde(rename = "servers.console")]
    ServersConsole,
    #[serde(rename = "templates.read")]
    TemplatesRead,
    #[serde(rename = "templates.write")]
    TemplatesWrite,
    #[serde(rename = "backups.read")]
    BackupsRead,
    #[serde(rename = "backups.write")]
    BackupsWrite,
}

impl Permission {
    pub const ALL: &'static [Permission] = &[
        Self::PlatformSetup,
        Self::PlatformSettingsRead,
        Self::PlatformSettingsWrite,
        Self::IdentityUsersRead,
        Self::IdentityUsersWrite,
        Self::IdentitySessionsRevoke,
        Self::NodesRead,
        Self::NodesEnroll,
        Self::NodesWrite,
        Self::AuditRead,
        Self::DiagnosticsRead,
        Self::ServersRead,
        Self::ServersWrite,
        Self::ServersConsole,
        Self::TemplatesRead,
        Self::TemplatesWrite,
        Self::BackupsRead,
        Self::BackupsWrite,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlatformSetup => "platform.setup",
            Self::PlatformSettingsRead => "platform.settings.read",
            Self::PlatformSettingsWrite => "platform.settings.write",
            Self::IdentityUsersRead => "identity.users.read",
            Self::IdentityUsersWrite => "identity.users.write",
            Self::IdentitySessionsRevoke => "identity.sessions.revoke",
            Self::NodesRead => "nodes.read",
            Self::NodesEnroll => "nodes.enroll",
            Self::NodesWrite => "nodes.write",
            Self::AuditRead => "audit.read",
            Self::DiagnosticsRead => "diagnostics.read",
            Self::ServersRead => "servers.read",
            Self::ServersWrite => "servers.write",
            Self::ServersConsole => "servers.console",
            Self::TemplatesRead => "templates.read",
            Self::TemplatesWrite => "templates.write",
            Self::BackupsRead => "backups.read",
            Self::BackupsWrite => "backups.write",
        }
    }

    /// Permissions granted to a role. Scoped grants will overlay this map later.
    pub fn granted_to(role: Role) -> &'static [Permission] {
        match role {
            Role::Owner | Role::Administrator => Self::ALL,
            Role::Operator => &[
                Self::PlatformSettingsRead,
                Self::IdentityUsersRead,
                Self::NodesRead,
                Self::NodesEnroll,
                Self::AuditRead,
                Self::DiagnosticsRead,
                Self::ServersRead,
                Self::ServersWrite,
                Self::ServersConsole,
                Self::TemplatesRead,
                Self::BackupsRead,
                Self::BackupsWrite,
            ],
            Role::Viewer => &[
                Self::PlatformSettingsRead,
                Self::IdentityUsersRead,
                Self::NodesRead,
                Self::AuditRead,
                Self::DiagnosticsRead,
                Self::ServersRead,
                Self::TemplatesRead,
                Self::BackupsRead,
            ],
        }
    }

    pub fn role_has(role: Role, permission: Permission) -> bool {
        Self::granted_to(role).contains(&permission)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RolePermissions {
    pub role: Role,
    pub permissions: Vec<Permission>,
}

impl RolePermissions {
    pub fn for_role(role: Role) -> Self {
        Self {
            role,
            permissions: Permission::granted_to(role).to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_cannot_enroll_nodes() {
        assert!(!Permission::role_has(Role::Viewer, Permission::NodesEnroll));
        assert!(Permission::role_has(Role::Owner, Permission::NodesEnroll));
    }

    #[test]
    fn identifiers_are_stable() {
        assert_eq!(Permission::NodesRead.as_str(), "nodes.read");
        assert_eq!(Permission::ALL.len(), 18);
    }
}
