//! Shared domain models. Keep this crate free of I/O so rules stay testable.

pub mod addon;
pub mod backup;
pub mod error;
pub mod ids;
pub mod job;
pub mod node;
pub mod permissions;
pub mod roles;
pub mod server;
pub mod template;
pub mod user;

pub use addon::{AddonInstallStatus, ServerAddonSummary};
pub use backup::{BackupStatus, BackupSummary};
pub use error::{ErrorCode, PlatformError};
pub use ids::*;
pub use job::{JobKind, JobStatus};
pub use node::{DockerState, NodeHealth, NodeStatus, ObservedResources};
pub use permissions::{Permission, RolePermissions};
pub use roles::Role;
pub use server::{AllocatedPort, ServerStatus, ServerSummary};
pub use template::{PortMapping, TemplateSource, TemplateSummary};
pub use user::{UserStatus, UserSummary};

pub const API_VERSION: &str = "v1";
pub const NODE_PROTOCOL_VERSION: u16 = 1;
pub const DATABASE_SCHEMA_VERSION: u32 = 5;
