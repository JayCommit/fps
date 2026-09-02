-- Schema version 5: host telemetry, remote node settings, uninstall from the panel.

ALTER TABLE nodes
    ADD COLUMN load_one FLOAT NULL AFTER disk_available_bytes,
    ADD COLUMN cpu_percent DOUBLE NULL AFTER load_one,
    ADD COLUMN memory_used_bytes BIGINT NULL AFTER cpu_percent,
    ADD COLUMN uptime_seconds BIGINT NULL AFTER memory_used_bytes,
    ADD COLUMN heartbeat_interval_seconds INT NOT NULL DEFAULT 15 AFTER uptime_seconds,
    ADD COLUMN docker_prune_requested TINYINT(1) NOT NULL DEFAULT 0 AFTER heartbeat_interval_seconds,
    ADD COLUMN uninstall_requested_at DATETIME(3) NULL AFTER docker_prune_requested,
    ADD COLUMN uninstalled_at DATETIME(3) NULL AFTER uninstall_requested_at;
