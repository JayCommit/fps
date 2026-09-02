# Recovery

Database schema version is **`3`** (`DATABASE_SCHEMA_VERSION`, migration
`services/control-plane/migrations/0003_workloads.sql`). Restore the MariaDB dump
**and** `/var/lib/fps` (CA key, data dir) together. A mismatched schema
or missing CA key will not re-enroll nodes automatically.

Authentication is Bearer only. Cookie sessions are rejected.

## Lost control plane process

1. Restore `/var/lib/fps` (CA material, data dir) and the MariaDB dump
   taken at schema version 3.
2. Install the same binary version (`0.0.1-alpha.1`).
3. Write systemd units (`fps install --role control-plane` or
   `fps install-artifacts --out ./out --role control-plane`) and start
   `fps-control-plane.service`. On the Ubuntu/Debian host you can instead re-run
   `deploy/install.sh --role control-plane --yes`.
4. Healthy nodes reconnect with their stored identity (mTLS on the node bind;
   bearer HTTP only when `FPS_ALLOW_INSECURE_HTTP` is set) and resume
   heartbeats. Queued jobs are delivered on the next heartbeat response.

## Lost node

1. Isolate or decommission the old VM. Do not reuse its client certificate.
2. Issue a new enrollment token (`POST /v1/nodes/enrollment-tokens`).
3. Enroll a replacement VM. Game containers on the lost disk are not migrated.
4. Revoke the lost node so a stale agent cannot heartbeat:
   `POST /v1/nodes/{id}/revoke` (`nodes.write`). This sets `nodes.revoked_at`.

## Compromised node

1. Take the host off the network. Stop `fps-node-agent.service`.
2. `POST /v1/nodes/{id}/revoke`. Heartbeats and job dispatch for that certificate
   and token are rejected.
3. Rotate the control-plane master key only if it may have leaked onto that
   node (it should not; the agent does not receive it).
4. Treat game data on that node as untrusted until restored from a known-good
   backup.

## Backups

Game-server archives are tracked in `backups`. The agent writes a gzip archive
under the node data dir (`backups/{id}.tgz`) and reports path and size on the
next heartbeat. Restoring is `JobKind::Restore` plus operator verification that
the container starts. Logical MariaDB dumps remain the control-plane recovery
path.

## Database

Take logical dumps of MariaDB before upgrades. After restore, confirm `/version`
reports `database_schema: 3`.
