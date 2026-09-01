pub mod apply;
pub mod config;
pub mod install;
pub mod plan;
pub mod preflight;
pub mod proxmox;

pub use config::{BootstrapConfig, GuestSpec, ProxmoxEndpoint};
pub use plan::{DeploymentPlan, PlanAction};
pub use preflight::{run_preflight, PreflightReport};
