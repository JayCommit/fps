use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use fps_branding::{DISPLAY_NAME, VERSION};
use fps_node_agent::{enroll, load_identity, run_heartbeat_loop, AgentConfig};
use fps_observability::{init_tracing, LogFormat};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "fps-node-agent", version = VERSION, about = DISPLAY_NAME)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Exchange a one-time enrollment token for node identity material.
    Enroll {
        #[arg(long, env = "FPS_CONTROL_PLANE_URL")]
        url: String,
        #[arg(long, env = "FPS_ENROLLMENT_TOKEN")]
        token: String,
        #[arg(long, env = "FPS_AGENT_DATA_DIR", default_value = "./data/agent")]
        data_dir: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, env = "FPS_ALLOW_INSECURE_HTTP", default_value_t = false)]
        allow_insecure_http: bool,
    },
    /// Run the heartbeat loop using previously stored identity.
    Run {
        #[arg(long, env = "FPS_AGENT_DATA_DIR", default_value = "./data/agent")]
        data_dir: PathBuf,
        #[arg(long, env = "FPS_ALLOW_INSECURE_HTTP", default_value_t = false)]
        allow_insecure_http: bool,
    },
    /// Print local diagnostics (no secrets).
    Doctor {
        #[arg(long, env = "FPS_AGENT_DATA_DIR", default_value = "./data/agent")]
        data_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LogFormat::Pretty, "info,fps_node_agent=debug");
    let cli = Cli::parse();
    match cli.command {
        Command::Enroll {
            url,
            token,
            data_dir,
            name,
            allow_insecure_http,
        } => {
            let cfg = AgentConfig {
                control_plane_url: url,
                data_dir,
                name,
                labels: vec!["role:game-node".into()],
                heartbeat_interval: Duration::from_secs(15),
                allow_insecure_http: allow_insecure_http || env_flag("FPS_ALLOW_INSECURE_HTTP"),
            };
            let identity = enroll(&cfg, &token).await?;
            println!("enrolled node_id={}", identity.node_id);
            Ok(())
        }
        Command::Run {
            data_dir,
            allow_insecure_http,
        } => {
            let identity = load_identity(&data_dir)?;
            let allow_insecure_http = allow_insecure_http
                || env_flag("FPS_ALLOW_INSECURE_HTTP")
                || identity.allows_insecure_http();
            let cfg = AgentConfig {
                control_plane_url: identity.control_plane_url.clone(),
                data_dir,
                name: None,
                labels: vec![],
                heartbeat_interval: Duration::from_secs(identity.heartbeat_interval_seconds),
                allow_insecure_http,
            };
            info!(node_id = %identity.node_id, "starting heartbeat loop");
            run_heartbeat_loop(&cfg, &identity).await
        }
        Command::Doctor { data_dir } => {
            let docker = fps_node_agent::docker::probe().await;
            println!(
                "agent_version={} docker={:?} engine={:?}",
                VERSION, docker.state, docker.engine_version
            );
            match load_identity(&data_dir) {
                Ok(id) => println!("identity=present node_id={}", id.node_id),
                Err(_) => println!("identity=missing"),
            }
            Ok(())
        }
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}
