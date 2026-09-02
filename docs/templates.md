# Templates

Native templates are first-class documents in `crates/templates`. Pterodactyl
Eggs are an import source only; the control plane never runs Egg installers as
host scripts.

## Native schema

- Kind: `fps.template`
- `schema_version: 1`
- Required: `name`, `slug` (lowercase alphanumeric plus hyphen), `docker_image`
- Optional: `game` (icon key), `startup`, `environment`, `ports`, `memory_mb`,
  `cpu_shares`, `volume_path` (default `/data`)

MariaDB schema 4 stores the catalogue in `templates` (`source` is `native` or
`egg_import`). Schema 6 adds `server_addons` for panel-installed plugins. The
control plane seeds the catalogue on startup if slugs are
missing, so existing installs pick up new games on the next process start.

| Slug | Game | Image | Notes |
|---|---|---|---|
| `http-echo` | demo | `hashicorp/http-echo:1.0.0` | CI / local deploy test |
| `minecraft-itzg` | minecraft | `itzg/minecraft-server:java21` | Vanilla; `EULA=TRUE` |
| `minecraft-paper` | minecraft | `itzg/minecraft-server:java21` | Paper |
| `minecraft-bedrock` | minecraft | `itzg/minecraft-bedrock-server` | Bedrock UDP 19132 |
| `fivem-txadmin` | fivem | `spritsail/fivem` | txAdmin on 40120; needs `LICENSE_KEY` |
| `cs2` | cs2 | `joedwards32/cs2` | Needs Steam `SRCDS_TOKEN` |
| `rust` | rust | `didstopia/rust-server` | Set RCON password |
| `valheim` | valheim | `lloesche/valheim-server` | Password ≥ 5 chars |
| `palworld` | palworld | `thijsvanloef/palworld-server-docker` | |
| `factorio` | factorio | `factoriotools/factorio:stable` | |
| `terraria` | terraria | `ryshe/terraria` | |
| `gmod` | gmod | `cm2network/gmod` | Needs `SRCDS_TOKEN` |
| `teamspeak` | teamspeak | `teamspeak` | Accept license |
| `satisfactory` | satisfactory | `wolveix/satisfactory-server` | |

Community images still require the operator to accept EULAs, provide CFX/Steam
tokens, and open the matching game ports on the host firewall.

## Addons

The panel can install and remove curated addons into a server volume. The
catalogue lives in `crates/templates/src/addons.rs` (same crate as native
templates). Operators click **Install** / **Uninstall** on the server page;
the control plane enqueues `addon_install` / `addon_uninstall` jobs and the
node agent downloads the archive into the server's Docker volume.

| Game | Addons |
|---|---|
| CS2 | MetaMod:Source, CounterStrikeSharp (needs MetaMod), SwiftlyS2 |
| Rust | Oxide / uMod |
| Minecraft Paper | LuckPerms, Vault, EssentialsX, PlaceholderAPI, spark, WorldEdit |
| FiveM | oxmysql, ox_lib, ox_core, qb-core, es_extended |
| Garry's Mod | ULib, ULX |

Vanilla Minecraft, Bedrock, and the HTTP Echo demo have an empty addon list.
Paper plugins are limited to the `minecraft-paper` template.

Downloads come from GitHub Releases, GitHub source archives, or an index page
(MetaMod 2.0 linux tarballs on `mms.alliedmods.net`). The agent refuses path
traversal and stays inside the server volume. Uninstall deletes the tracked
paths recorded at install time and reverses `gameinfo.gi` line patches.

`GET /v1/addons?game=cs2` lists the catalogue. `GET /v1/servers/{id}/addons`
merges that list with install state. Install requires `servers.write` and
auto-queues missing dependencies (CounterStrikeSharp installs MetaMod first).

## Interpolation

`interpolate` replaces `{{VAR}}` and `${VAR}`. Create-server merges template
environment with operator overrides and sets `SERVER_NAME` to the instance
name. Game images keep their own listen-port variables (for example itzg
`SERVER_PORT=25565` inside the container). Host publishes use each game's
real default port when it is free.

## Egg import

`POST /v1/templates/import-egg` accepts a Pterodactyl/Pelican Egg JSON object
and stores a native template (`import_egg` in `crates/templates`).

## Deploy path

`POST /v1/servers` picks an online node with Docker available, publishes each
template port on the matching host port (Minecraft `25565`, CS2 `27015` /
`27020`, FiveM `30120` / `40120`, …). If that bind is already taken on the
node, the next free port is used. An `install` job pulls and starts the
image; pull progress is streamed to the server console. Start, stop, delete,
backup, file listing, and interval schedules use the same job channel. A
Docker “port is already allocated” error reallocates and retries instead of
leaving the server failed with a raw engine 500.
