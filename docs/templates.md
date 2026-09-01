# Templates

Native templates are first-class documents in `crates/templates`. Pterodactyl
Eggs are an import source only; the control plane never runs Egg installers as
host scripts.

## Native schema

- Kind: `fps.template`
- `schema_version: 1`
- Required: `name`, `slug` (lowercase alphanumeric plus hyphen), `docker_image`
- Optional: `startup`, `environment`, `ports`, `memory_mb`, `cpu_shares`,
  `volume_path` (default `/data`)

MariaDB schema 3 stores the catalogue in `templates` (`source` is `native` or
`egg_import`). The control plane seeds two catalogue entries on startup:

| Slug | Image | Purpose |
|---|---|
| `http-echo` | `hashicorp/http-echo:1.0.0` | Demo workload used in automated tests |
| `minecraft-itzg` | `itzg/minecraft-server:java21` | Vanilla Minecraft; `EULA=TRUE` required |

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
