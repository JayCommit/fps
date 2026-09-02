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
`egg_import`). The control plane seeds the catalogue on startup if slugs are
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

## Interpolation

`interpolate` replaces `{{VAR}}` and `${VAR}`. Create-server merges template
environment with operator overrides and injects `SERVER_PORT` / `SERVER_NAME`.

## Egg import

`POST /v1/templates/import-egg` accepts a Pterodactyl/Pelican Egg JSON object
and stores a native template (`import_egg` in `crates/templates`).

## Deploy path

`POST /v1/servers` picks an online node with Docker available, allocates a host
port in 25000–25999, enqueues an `install` job, and the agent pulls/runs the
image on the next heartbeat. Start, stop, backup, file listing, and interval
schedules are the same job channel.
