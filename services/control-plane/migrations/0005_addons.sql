-- Schema version 5: per-server addon installs (mod loaders, plugins, resources).

CREATE TABLE server_addons (
    id CHAR(36) NOT NULL,
    server_id CHAR(36) NOT NULL,
    addon_slug VARCHAR(64) NOT NULL,
    addon_name VARCHAR(160) NOT NULL,
    version_label VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    tracked_paths_json JSON NOT NULL,
    spec_json JSON NOT NULL,
    job_id CHAR(36) NULL,
    error TEXT NULL,
    installed_at DATETIME(3) NULL,
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uniq_server_addon (server_id, addon_slug),
    KEY idx_server_addons_server (server_id),
    CONSTRAINT fk_server_addons_server FOREIGN KEY (server_id) REFERENCES servers (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
