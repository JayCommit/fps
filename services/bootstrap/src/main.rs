use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use fps_bootstrap::apply::apply_guests;
use fps_bootstrap::config::BootstrapConfig;
use fps_bootstrap::install::{perform_host_install, HostInstallOpts};
use fps_bootstrap::plan::build_plan;
use fps_bootstrap::preflight::run_preflight;
use fps_bootstrap::proxmox::{HttpProxmox, ProxmoxView};
use fps_bootstrap::role::InstallRole;
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
    /// Install this machine as the control plane, a game host, or both.
    Install(InstallArgs),
    /// Sign in to a running control plane and store a session under ~/.config/fps.
    Login {
        #[arg(long, env = "FPS_PUBLIC_URL")]
        url: String,
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Forget the saved operator session.
    Logout,
    /// Print control-plane /version using the saved session.
    Status,
    /// List servers.
    Servers,
    /// List nodes.
    Nodes,
    /// Check GitHub Releases for a newer version (never /releases/latest).
    CheckUpdate,
    /// Optional Proxmox guest create via the HTTP API.
    /// Does not install FPS inside the guest — use `deploy/install.sh` on Ubuntu/Debian for that.
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
        #[arg(long, default_value = "https://github.com/JayCommit/fps/releases")]
        release_notes_url: String,
    },
    /// Write systemd units and env templates. Does not start services or contact Proxmox.
    InstallArtifacts {
        #[arg(long)]
        out: PathBuf,
        /// control-plane, game-host, or both (default both).
        #[arg(long, value_parser = parse_role)]
        role: Option<InstallRole>,
    },
}

#[derive(Debug, clap::Args)]
struct InstallArgs {
    /// control-plane (web), game-host (Homer), or both. Asked interactively if omitted.
    #[arg(long, value_parser = parse_role)]
    role: Option<InstallRole>,
    /// systemctl enable --now (default: write files only). Ignored with --destdir.
    #[arg(long)]
    start: bool,
    /// Prefix all install paths (packaging / tests). Never starts units.
    #[arg(long)]
    destdir: Option<PathBuf>,
    /// Directory to search for fps-control-plane / fps-node-agent / fps.
    #[arg(long)]
    bin_dir: Option<PathBuf>,
    /// Binary prefix (default /opt/fps).
    #[arg(long, default_value = "/opt/fps")]
    prefix: PathBuf,
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
        #[arg(long, value_parser = parse_role, default_value_t = InstallRole::Both)]
        role: InstallRole,
    },
    /// Apply the plan. Refuses to run without --yes. Real hosts also require FPS_ALLOW_REAL_PROXMOX=1.
    Apply {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        fake_base: Option<String>,
        #[arg(long, value_parser = parse_role, default_value_t = InstallRole::Both)]
        role: InstallRole,
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
        #[arg(long, value_parser = parse_role, default_value_t = InstallRole::Both)]
        role: InstallRole,
    },
    Upgrade {
        #[arg(long)]
        config: PathBuf,
    },
    UninstallPlan {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, value_parser = parse_role, default_value_t = InstallRole::Both)]
        role: InstallRole,
    },
}

fn parse_role(s: &str) -> Result<InstallRole, String> {
    InstallRole::from_str(s).map_err(|e| e.to_string())
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
        Command::Login {
            url,
            email,
            password,
        } => {
            let password = match password {
                Some(p) => p,
                None => fps_bootstrap::ops::prompt_password()?,
            };
            let session = fps_bootstrap::ops::login(&url, &email, &password).await?;
            println!("signed in as {} @ {}", session.email, session.url);
            Ok(())
        }
        Command::Logout => {
            fps_bootstrap::ops::delete_session()?;
            println!("signed out");
            Ok(())
        }
        Command::Status => {
            let body = fps_bootstrap::ops::get_json("/version").await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
            Ok(())
        }
        Command::Servers => {
            let body = fps_bootstrap::ops::get_json("/v1/servers").await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
            Ok(())
        }
        Command::Nodes => {
            let body = fps_bootstrap::ops::get_json("/v1/nodes").await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
            Ok(())
        }
        Command::CheckUpdate => {
            let body = fps_bootstrap::ops::get_json("/v1/updates/check").await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
            Ok(())
        }
        Command::Install(args) => {
            let role = InstallRole::prompt_or_require_flag(args.role)?;
            let report = perform_host_install(&HostInstallOpts {
                role,
                start: args.start,
                destdir: args.destdir,
                bin_dir: args.bin_dir,
                prefix: args.prefix,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            eprintln!("\nInstalled as {}.", role.title());
            for step in &report.next_steps {
                eprintln!("  → {step}");
            }
            Ok(())
        }
        Command::InstallArtifacts { out, role } => {
            let role = role.unwrap_or(InstallRole::Both);
            fps_bootstrap::install::write_install_artifacts(&out, role)?;
            println!(
                "{}",
                serde_json::json!({
                    "wrote": out.display().to_string(),
                    "plan": fps_bootstrap::install::install_plan(role),
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
            BootstrapCommand::Plan {
                config,
                fake_base,
                role,
            } => {
                let cfg = BootstrapConfig::load(&config)?;
                cfg.require_for_role(role)?;
                let plan = build_plan(&cfg, true, role);
                println!("{}", serde_json::to_string_pretty(&plan)?);
                if let Some(base) = fake_base {
                    let (c, g) = clients(&cfg, Some(&base), role)?;
                    let report = run_preflight(&cfg, c.as_ref(), g.as_ref(), role).await?;
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
                role,
            } => {
                if !yes {
                    bail!("apply requires --yes after reviewing the plan. Nothing was changed.");
                }
                let cfg = BootstrapConfig::load(&config)?;
                cfg.require_for_role(role)?;
                let (c, g) = clients(&cfg, fake_base.as_deref(), role)?;
                let report = run_preflight(&cfg, c.as_ref(), g.as_ref(), role).await?;
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
                let upids = apply_guests(&cfg, c.as_ref(), g.as_ref(), role).await?;
                println!(
                    "{}",
                    serde_json::json!({
                        "applied": true,
                        "role": role,
                        "upids": upids,
                        "next": "SSH into the guest and run: fps install --role ".to_string() + role.as_str(),
                    })
                );
                Ok(())
            }
            BootstrapCommand::Status { config } => {
                let cfg = BootstrapConfig::load(&config)?;
                println!(
                    "configured control_plane={} game_node={}",
                    cfg.control_plane
                        .as_ref()
                        .map(|g| g.hostname.as_str())
                        .unwrap_or("(none)"),
                    cfg.game_node
                        .as_ref()
                        .map(|g| g.hostname.as_str())
                        .unwrap_or("(none)")
                );
                Ok(())
            }
            BootstrapCommand::Doctor {
                config,
                fake_base,
                role,
            } => {
                let cfg = BootstrapConfig::load(&config)?;
                cfg.require_for_role(role)?;
                if let Some(base) = fake_base {
                    let (c, g) = clients(&cfg, Some(&base), role)?;
                    let report = run_preflight(&cfg, c.as_ref(), g.as_ref(), role).await?;
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
                match fps_bootstrap::ops::get_json("/v1/updates/check").await {
                    Ok(body) => {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                        Ok(())
                    }
                    Err(_) => {
                        println!(
                            "upgrade: sign in with `fps login` then `fps check-update`. Binary replacement + sqlx migrate remains the control-plane upgrade path."
                        );
                        Ok(())
                    }
                }
            }
            BootstrapCommand::UninstallPlan { config, role } => {
                let cfg = BootstrapConfig::load(&config)?;
                let cp = cfg
                    .control_plane
                    .as_ref()
                    .filter(|_| role.includes_control_plane())
                    .map(|g| g.vmid.to_string())
                    .unwrap_or_else(|| "—".into());
                let gn = cfg
                    .game_node
                    .as_ref()
                    .filter(|_| role.includes_game_host())
                    .map(|g| g.vmid.to_string())
                    .unwrap_or_else(|| "—".into());
                println!(
                    "Would stop FPS services for role {role} and leave guests {cp} / {gn} in place. This command never deletes VMs."
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
    role: InstallRole,
) -> Result<(Box<dyn ProxmoxView>, Box<dyn ProxmoxView>)> {
    if let Some(base) = fake_base {
        let c = HttpProxmox::new(base)?;
        let g = HttpProxmox::new(base)?;
        return Ok((Box::new(c), Box::new(g)));
    }
    let control: Box<dyn ProxmoxView> = if role.includes_control_plane() {
        let guest = cfg
            .control_plane
            .as_ref()
            .context("control-plane guest required for this role")?;
        let secret = std::env::var(&guest.proxmox.token_secret_env).unwrap_or_default();
        Box::new(fps_bootstrap::proxmox::ProxmoxClient::new(
            &guest.proxmox.url,
            &guest.proxmox.token_id,
            &secret,
            true,
        )?)
    } else {
        Box::new(fps_bootstrap::proxmox::UnusedProxmox)
    };
    let game: Box<dyn ProxmoxView> = if role.includes_game_host() {
        let guest = cfg
            .game_node
            .as_ref()
            .context("game-host guest required for this role")?;
        let secret = std::env::var(&guest.proxmox.token_secret_env).unwrap_or_default();
        Box::new(fps_bootstrap::proxmox::ProxmoxClient::new(
            &guest.proxmox.url,
            &guest.proxmox.token_id,
            &secret,
            true,
        )?)
    } else {
        Box::new(fps_bootstrap::proxmox::UnusedProxmox)
    };
    Ok((control, game))
}
