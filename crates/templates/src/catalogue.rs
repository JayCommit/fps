//! Seeded native templates. Images are well-known community Docker images;
//! operators still supply licenses, Steam tokens, and EULAs.

use std::collections::BTreeMap;

use crate::{NativePort, NativeTemplate, NATIVE_TEMPLATE_KIND};

fn port(name: &str, protocol: &str, container_port: u16) -> NativePort {
    NativePort {
        name: name.into(),
        protocol: protocol.into(),
        container_port,
    }
}

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn native(
    name: &str,
    slug: &str,
    game: &str,
    description: &str,
    docker_image: &str,
    memory_mb: u32,
    cpu_shares: u32,
    volume_path: &str,
    environment: BTreeMap<String, String>,
    ports: Vec<NativePort>,
) -> NativeTemplate {
    NativeTemplate {
        kind: NATIVE_TEMPLATE_KIND.into(),
        schema_version: 1,
        name: name.into(),
        slug: slug.into(),
        game: game.into(),
        description: description.into(),
        docker_image: docker_image.into(),
        startup: None,
        environment,
        ports,
        memory_mb,
        cpu_shares,
        volume_path: volume_path.into(),
    }
}

pub fn http_echo() -> NativeTemplate {
    native(
        "HTTP Echo",
        "http-echo",
        "demo",
        "Tiny demo workload (hashicorp/http-echo) used for local and CI deploy tests.",
        "hashicorp/http-echo:1.0.0",
        64,
        256,
        "/data",
        env(&[("ECHO_TEXT", "fps")]),
        vec![port("http", "tcp", 5678)],
    )
}

pub fn minecraft_vanilla() -> NativeTemplate {
    native(
        "Minecraft (Vanilla)",
        "minecraft-itzg",
        "minecraft",
        "Vanilla Minecraft via itzg/minecraft-server. Accept the EULA in environment before players join.",
        "itzg/minecraft-server:java21",
        2048,
        1024,
        "/data",
        env(&[("EULA", "TRUE"), ("TYPE", "VANILLA"), ("MEMORY", "2G")]),
        vec![port("game", "tcp", 25565)],
    )
}

pub fn minecraft_paper() -> NativeTemplate {
    native(
        "Minecraft (Paper)",
        "minecraft-paper",
        "minecraft",
        "Paper on Java 21 via itzg/minecraft-server. Plugin-friendly default for survival and minigames.",
        "itzg/minecraft-server:java21",
        2048,
        1024,
        "/data",
        env(&[
            ("EULA", "TRUE"),
            ("TYPE", "PAPER"),
            ("MEMORY", "2G"),
            ("USE_AIKAR_FLAGS", "true"),
        ]),
        vec![port("game", "tcp", 25565)],
    )
}

pub fn minecraft_bedrock() -> NativeTemplate {
    native(
        "Minecraft (Bedrock)",
        "minecraft-bedrock",
        "minecraft",
        "Dedicated Bedrock server (itzg). Console/phone/Win10 clients; not Java edition.",
        "itzg/minecraft-bedrock-server:latest",
        1024,
        1024,
        "/data",
        env(&[
            ("EULA", "TRUE"),
            ("GAMEMODE", "survival"),
            ("DIFFICULTY", "normal"),
        ]),
        vec![port("game", "udp", 19132)],
    )
}

pub fn fivem_txadmin() -> NativeTemplate {
    native(
        "FiveM (txAdmin)",
        "fivem-txadmin",
        "fivem",
        "CitizenFX FiveM with txAdmin. Set LICENSE_KEY from portal.cfx.re; open 30120 (game) and 40120 (txAdmin).",
        "spritsail/fivem:latest",
        4096,
        2048,
        "/config",
        env(&[
            ("LICENSE_KEY", "change-me"),
            ("TXADMIN", "1"),
            ("TXADMIN_PORT", "40120"),
        ]),
        vec![
            port("game", "tcp", 30120),
            port("game-udp", "udp", 30120),
            port("txadmin", "tcp", 40120),
        ],
    )
}

pub fn cs2() -> NativeTemplate {
    native(
        "Counter-Strike 2",
        "cs2",
        "cs2",
        "CS2 dedicated server (joedwards32). Requires a Steam Game Server token in SRCDS_TOKEN.",
        "joedwards32/cs2:latest",
        4096,
        2048,
        "/home/steam/cs2-dedicated",
        env(&[
            ("SRCDS_TOKEN", "change-me"),
            ("CS2_SERVERNAME", "FPS CS2"),
            ("CS2_MAXPLAYERS", "12"),
            ("CS2_RCONPW", "changeme"),
            ("CS2_LAN", "0"),
        ]),
        vec![
            port("game", "tcp", 27015),
            port("game-udp", "udp", 27015),
            port("sourcetv", "udp", 27020),
        ],
    )
}

pub fn rust() -> NativeTemplate {
    native(
        "Rust",
        "rust",
        "rust",
        "Rust dedicated server. Set RUST_RCON_PASSWORD and world identity before going public.",
        "didstopia/rust-server:latest",
        6144,
        2048,
        "/steamcmd/rust",
        env(&[
            ("RUST_SERVER_NAME", "FPS Rust"),
            ("RUST_SERVER_IDENTITY", "fps-rust"),
            ("RUST_SERVER_MAXPLAYERS", "50"),
            ("RUST_RCON_PASSWORD", "changeme"),
            ("RUST_RCON_PORT", "28016"),
            ("RUST_SERVER_WORLDSIZE", "3500"),
        ]),
        vec![port("game", "udp", 28015), port("rcon", "tcp", 28016)],
    )
}

pub fn valheim() -> NativeTemplate {
    native(
        "Valheim",
        "valheim",
        "valheim",
        "Valheim dedicated server. SERVER_PASS must be at least 5 characters.",
        "lloesche/valheim-server:latest",
        4096,
        2048,
        "/config",
        env(&[
            ("SERVER_NAME", "FPS Valheim"),
            ("WORLD_NAME", "Dedicated"),
            ("SERVER_PASS", "secret"),
            ("SERVER_PUBLIC", "false"),
        ]),
        vec![
            port("game", "udp", 2456),
            port("query", "udp", 2457),
            port("steam", "udp", 2458),
        ],
    )
}

pub fn palworld() -> NativeTemplate {
    native(
        "Palworld",
        "palworld",
        "palworld",
        "Palworld dedicated server. Set ADMIN_PASSWORD before exposing the query port.",
        "thijsvanloef/palworld-server-docker:latest",
        8192,
        2048,
        "/palworld",
        env(&[
            ("SERVER_NAME", "FPS Palworld"),
            ("PLAYERS", "16"),
            ("ADMIN_PASSWORD", "changeme"),
            ("SERVER_PASSWORD", ""),
            ("COMMUNITY", "false"),
            ("RCON_ENABLED", "true"),
            ("RCON_PORT", "25575"),
        ]),
        vec![port("game", "udp", 8211), port("rcon", "tcp", 25575)],
    )
}

pub fn factorio() -> NativeTemplate {
    native(
        "Factorio",
        "factorio",
        "factorio",
        "Official Factorio dedicated server image. Saves live under /factorio.",
        "factoriotools/factorio:stable",
        2048,
        1024,
        "/factorio",
        env(&[("TOKEN", ""), ("USER", "")]),
        vec![port("game", "udp", 34197)],
    )
}

pub fn terraria() -> NativeTemplate {
    native(
        "Terraria",
        "terraria",
        "terraria",
        "Vanilla Terraria dedicated server.",
        "ryshe/terraria:latest",
        1024,
        1024,
        "/root/.local/share/Terraria/Worlds",
        env(&[
            ("WORLD", "fps.wld"),
            ("WORLDSIZE", "2"),
            ("DIFFICULTY", "0"),
            ("MAXPLAYERS", "8"),
        ]),
        vec![port("game", "tcp", 7777)],
    )
}

pub fn gmod() -> NativeTemplate {
    native(
        "Garry's Mod",
        "gmod",
        "gmod",
        "Garry's Mod dedicated server. Set GSLT (Steam game server login token) for public listing.",
        "cm2network/gmod:latest",
        2048,
        1024,
        "/home/steam/gmod-dedicated",
        env(&[
            ("SRCDS_TOKEN", "change-me"),
            ("HOSTNAME", "FPS GMod"),
            ("MAXPLAYERS", "16"),
            ("GAMEMODE", "sandbox"),
            ("MAP", "gm_flatgrass"),
        ]),
        vec![port("game", "tcp", 27015), port("game-udp", "udp", 27015)],
    )
}

pub fn teamspeak() -> NativeTemplate {
    native(
        "TeamSpeak 3",
        "teamspeak",
        "teamspeak",
        "Official TeamSpeak 3 server. Accept the license; query and file-transfer ports are published.",
        "teamspeak:latest",
        512,
        256,
        "/var/ts3server",
        env(&[("TS3SERVER_LICENSE", "accept")]),
        vec![
            port("voice", "udp", 9987),
            port("query", "tcp", 10011),
            port("files", "tcp", 30033),
        ],
    )
}

pub fn satisfactory() -> NativeTemplate {
    native(
        "Satisfactory",
        "satisfactory",
        "satisfactory",
        "Satisfactory dedicated server (wolveix). 1.0 listens on 7777 UDP and TCP; claim the server in-game.",
        "wolveix/satisfactory-server:latest",
        8192,
        2048,
        "/config",
        env(&[
            ("MAXPLAYERS", "4"),
            ("AUTOPAUSE", "true"),
            ("AUTOSAVEINTERVAL", "300"),
            ("SERVERGAMEPORT", "7777"),
        ]),
        vec![port("game", "udp", 7777), port("game-tcp", "tcp", 7777)],
    )
}

/// All templates seeded into a fresh control plane.
pub fn seeded() -> Vec<NativeTemplate> {
    vec![
        http_echo(),
        minecraft_vanilla(),
        minecraft_paper(),
        minecraft_bedrock(),
        fivem_txadmin(),
        cs2(),
        rust(),
        valheim(),
        palworld(),
        factorio(),
        terraria(),
        gmod(),
        teamspeak(),
        satisfactory(),
    ]
}
