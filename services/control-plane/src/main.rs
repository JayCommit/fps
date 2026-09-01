use anyhow::Context;
use clap::{Parser, Subcommand};
use fps_branding::{DISPLAY_NAME, PACKAGE_NAME, VERSION};
use fps_config::ControlPlaneConfig;
use fps_control_plane::{dump_openapi, dump_permissions, serve};
use fps_observability::{init_tracing, LogFormat};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "fps-control-plane", version = VERSION, about = DISPLAY_NAME)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the API and node mTLS listeners.
    Serve,
    /// Print the OpenAPI document to stdout.
    DumpOpenapi,
    /// Print canonical permission identifiers as JSON.
    DumpPermissions,
    /// Validate configuration and database connectivity.
    Doctor,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv_optional();
    let cli = Cli::parse();
    match cli.command {
        Command::DumpOpenapi => {
            println!("{}", dump_openapi());
            Ok(())
        }
        Command::DumpPermissions => {
            println!("{}", dump_permissions());
            Ok(())
        }
        Command::Doctor => {
            init_tracing(LogFormat::Pretty, "info");
            let cfg = ControlPlaneConfig::from_env()?;
            info!(target: "doctor", product = DISPLAY_NAME, package = PACKAGE_NAME, config = %cfg.redacted_diagnostics());
            let pool = fps_control_plane::db::connect(&cfg.database_url).await?;
            sqlx::query("SELECT 1").execute(&pool).await?;
            println!("ok database={}", fps_config::redact_url(&cfg.database_url));
            Ok(())
        }
        Command::Serve => {
            let cfg = ControlPlaneConfig::from_env()?;
            let format = cfg.log_format.parse().unwrap_or(LogFormat::Pretty);
            init_tracing(format, "info,fps_control_plane=debug,sqlx=warn");
            serve(cfg).await.context("serve")
        }
    }
}

fn dotenv_optional() {
    let _ = std::fs::read_to_string(".env").map(|contents| {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if std::env::var(k).is_err() {
                    std::env::set_var(k, v);
                }
            }
        }
    });
}
