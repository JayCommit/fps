use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use fps_bootstrap::apply::apply_guests;
use fps_bootstrap::config::BootstrapConfig;
use fps_bootstrap::plan::build_plan;
use fps_bootstrap::preflight::run_preflight;
use fps_bootstrap::proxmox::{HttpProxmox, ProxmoxView};
use fps_branding::{Channel, DISPLAY_NAME, VERSION};
use fps_observability::{init_tracing, LogFormat};
use fps_updater::{sign_manifest, signing_key_from_hex, ManifestAsset, UpdateManifest};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(name = "fps", version = VERSION, about = DISPLAY_NAME)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print product and protocol versions.
    Version,
    /// Proxmox bootstrap commands.
    Bootstrap {
        #[command(subcommand)]
        command: BootstrapCommand,
    },
    /// Build a canonical `update-manifest.json` and optional Ed25519 signature.
    ReleaseManifest {
        #[arg(long)]
        version: String,
        #[arg(long)]
        channel: String,
        #[arg(long)]
        git_tag: String,
        #[arg(long)]
        git_commit: String,
        #[arg(long)]
        artifacts_dir: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        signing_key_hex: Option<String>,
        #[arg(
            long,
            default_value = "https://github.com/JayCommit/fps/releases"
        )]
        release_notes_url: String,
    },
    /// Write systemd units and env templates. Does not start services or contact Proxmox.
    InstallArtifacts {
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum BootstrapCommand {
    /// Validate a deployment.toml without contacting Proxmox.
    Init {
        #[arg(long)]
        config: PathBuf,
    },
    /// Read-only preflight against the configured Proxmox APIs.
    Plan {
        #[arg(long)]
        config: PathBuf,
        /// Use an already-running fake/local Proxmox base URL (tests).
        #[arg(long)]
        fake_base: Option<String>,
    },
    /// Apply the plan. Refuses to run without --yes. Real hosts also require FPS_ALLOW_REAL_PROXMOX=1.
    Apply {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        fake_base: Option<String>,
    },
    Status {
        #[arg(long)]
        config: PathBuf,
    },
    Doctor {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        fake_base: Option<String>,
    },
    Upgrade {
        #[arg(long)]
        config: PathBuf,
    },
    UninstallPlan {
        #[arg(long)]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LogFormat::Pretty, "info");
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!(
                "{DISPLAY_NAME} {VERSION} (node-protocol {})",
                fps_domain::NODE_PROTOCOL_VERSION
            );
            Ok(())
        }
        Command::InstallArtifacts { out } => {
            fps_bootstrap::install::write_install_artifacts(&out)?;
            println!(
                "{}",
                serde_json::json!({
                    "wrote": out.display().to_string(),
                    "plan": fps_bootstrap::install::install_plan(),
                })
            );
            Ok(())
        }
        Command::ReleaseManifest {
            version,
            channel,
            git_tag,
            git_commit,
            artifacts_dir,
            output,
            signing_key_hex,
            release_notes_url,
        } => write_release_manifest(
            version,
            channel,
            git_tag,
            git_commit,
            artifacts_dir,
            output,
            signing_key_hex,
            release_notes_url,
        ),
        Command::Bootstrap { command } => match command {
            BootstrapCommand::Init { config } => {
                let cfg = BootstrapConfig::load(&config)?;
                println!("{}", serde_json::to_string_pretty(&cfg.redacted())?);
                Ok(())
            }
            BootstrapCommand::Plan { config, fake_base } => {
                let cfg = BootstrapConfig::load(&config)?;
                let plan = build_plan(&cfg, true);
                println!("{}", serde_json::to_string_pretty(&plan)?);
                if let Some(base) = fake_base {
                    let (c, g) = clients(&cfg, Some(&base))?;
                    let report = run_preflight(&cfg, c.as_ref(), g.as_ref()).await?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                    if !report.ok {
                        bail!("preflight failed");
                    }
                }
                Ok(())
            }
            BootstrapCommand::Apply {
                config,
                yes,
                fake_base,
            } => {
                if !yes {
                    bail!("apply requires --yes after reviewing the plan. Nothing was changed.");
                }
                let cfg = BootstrapConfig::load(&config)?;
                let (c, g) = clients(&cfg, fake_base.as_deref())?;
                let report = run_preflight(&cfg, c.as_ref(), g.as_ref()).await?;
                if !report.ok {
                    bail!(
                        "preflight failed; refusing to apply:\n{}",
                        serde_json::to_string_pretty(&report)?
                    );
                }
                if fake_base.is_none() {
                    match std::env::var("FPS_ALLOW_REAL_PROXMOX").as_deref() {
                        Ok("1") => {}
                        _ => {
                            bail!(
                                "real Proxmox apply requires FPS_ALLOW_REAL_PROXMOX=1 after reviewing the plan. Use --fake-base for tests."
                            );
                        }
                    }
                }
                let upids = apply_guests(&cfg, c.as_ref(), g.as_ref()).await?;
                println!(
                    "{}",
                    serde_json::json!({
                        "applied": true,
                        "upids": upids,
                    })
                );
                Ok(())
            }
            BootstrapCommand::Status { config } => {
                let cfg = BootstrapConfig::load(&config)?;
                println!(
                    "configured control_plane={} game_node={}",
                    cfg.control_plane.hostname, cfg.game_node.hostname
                );
                Ok(())
            }
            BootstrapCommand::Doctor { config, fake_base } => {
                let cfg = BootstrapConfig::load(&config)?;
                if let Some(base) = fake_base {
                    let (c, g) = clients(&cfg, Some(&base))?;
                    let report = run_preflight(&cfg, c.as_ref(), g.as_ref()).await?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                    if !report.ok {
                        bail!("doctor found failures");
                    }
                } else {
                    println!("config valid; pass --fake-base or live credentials to probe APIs");
                }
                Ok(())
            }
            BootstrapCommand::Upgrade { config } => {
                let _cfg = BootstrapConfig::load(&config)?;
                println!("upgrade: not available in 0.0.1-alpha.1 beyond binary replacement + sqlx migrate");
                Ok(())
            }
            BootstrapCommand::UninstallPlan { config } => {
                let cfg = BootstrapConfig::load(&config)?;
                println!(
                    "Would stop services and leave guests {} / {} in place. This command never deletes VMs.",
                    cfg.control_plane.vmid, cfg.game_node.vmid
                );
                Ok(())
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn write_release_manifest(
    version: String,
    channel: String,
    git_tag: String,
    git_commit: String,
    artifacts_dir: PathBuf,
    output: PathBuf,
    signing_key_hex: Option<String>,
    release_notes_url: String,
) -> Result<()> {
    let channel = match channel.as_str() {
        "alpha" => Channel::Alpha,
        "beta" => Channel::Beta,
        "stable" => Channel::Stable,
        other => bail!("unknown channel '{other}'"),
    };
    let mut assets = Vec::new();
    for entry in std::fs::read_dir(&artifacts_dir)
        .with_context(|| format!("read artifacts dir {}", artifacts_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "SHA256SUMS" || name == "update-manifest.json" || name.ends_with(".sig") {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        assets.push(ManifestAsset {
            name: name.clone(),
            url: format!("{release_notes_url}/download/{git_tag}/{name}"),
            size: bytes.len() as u64,
            sha256,
            content_type: "application/octet-stream".into(),
            platform: infer_platform(&name),
        });
    }
    assets.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = UpdateManifest {
        schema_version: 1,
        product_version: version,
        channel,
        git_tag,
        git_commit,
        published_at: chrono::Utc::now().to_rfc3339(),
        release_notes_url,
        min_control_plane: VERSION.to_string(),
        min_node_protocol: fps_domain::NODE_PROTOCOL_VERSION,
        min_desktop: "0.0.1-alpha.5".into(),
        min_bootstrap: VERSION.to_string(),
        min_database_schema: fps_domain::DATABASE_SCHEMA_VERSION,
        assets,
        migrations_required: true,
        restart_required: true,
        rollback_supported: false,
    };
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let key_hex = signing_key_hex
        .or_else(|| std::env::var("RELEASE_SIGNING_KEY").ok())
        .filter(|s| !s.is_empty());
    let Some(key_hex) = key_hex else {
        bail!("RELEASE_SIGNING_KEY or --signing-key-hex is required to emit a signed manifest");
    };
    let key = signing_key_from_hex(&key_hex).map_err(|e| anyhow::anyhow!("{e}"))?;
    let (canonical, signature_hex) =
        sign_manifest(&manifest, &key).map_err(|e| anyhow::anyhow!("{e}"))?;
    std::fs::write(&output, &canonical)?;
    std::fs::write(
        format!("{}.sig", output.display()),
        signature_hex.as_bytes(),
    )?;
    println!(
        "wrote {} ({} bytes) and {}.sig",
        output.display(),
        canonical.len(),
        output.display()
    );
    Ok(())
}

fn infer_platform(name: &str) -> String {
    if name.contains("windows") {
        "windows-x86_64".into()
    } else if name.contains("aarch64") || name.contains("arm64") {
        "linux-aarch64".into()
    } else {
        "linux-x86_64".into()
    }
}

fn clients(
    cfg: &BootstrapConfig,
    fake_base: Option<&str>,
) -> Result<(Box<dyn ProxmoxView>, Box<dyn ProxmoxView>)> {
    if let Some(base) = fake_base {
        let c = HttpProxmox::new(base)?;
        let g = HttpProxmox::new(base)?;
        return Ok((Box::new(c), Box::new(g)));
    }
    let c_secret = std::env::var(&cfg.control_plane.proxmox.token_secret_env).unwrap_or_default();
    let g_secret = std::env::var(&cfg.game_node.proxmox.token_secret_env).unwrap_or_default();
    let c = fps_bootstrap::proxmox::ProxmoxClient::new(
        &cfg.control_plane.proxmox.url,
        &cfg.control_plane.proxmox.token_id,
        &c_secret,
        true,
    )?;
    let g = fps_bootstrap::proxmox::ProxmoxClient::new(
        &cfg.game_node.proxmox.url,
        &cfg.game_node.proxmox.token_id,
        &g_secret,
        true,
    )?;
    Ok((Box::new(c), Box::new(g)))
}
