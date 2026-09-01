-- Schema version 3: identity ops leftovers + native workloads.

ALTER TABLE nodes
    ADD COLUMN revoked_at DATETIME(3) NULL AFTER updated_at;

CREATE TABLE templates (
    id CHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    slug VARCHAR(64) NOT NULL,
    description TEXT NOT NULL,
    docker_image VARCHAR(255) NOT NULL,
    startup_command TEXT NULL,
    env_json JSON NOT NULL,
    ports_json JSON NOT NULL,
    memory_mb INT NOT NULL DEFAULT 64,
    cpu_shares INT NOT NULL DEFAULT 1024,
    volume_path VARCHAR(255) NOT NULL DEFAULT '/data',
    source VARCHAR(32) NOT NULL,
    body_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_templates_slug (slug)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE allocations (
    id CHAR(36) NOT NULL,
    node_id CHAR(36) NOT NULL,
    ip VARCHAR(64) NOT NULL DEFAULT '0.0.0.0',
    port INT NOT NULL,
    protocol VARCHAR(8) NOT NULL DEFAULT 'tcp',
    notes VARCHAR(255) NULL,
    assigned_server_id CHAR(36) NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_alloc_bind (node_id, ip, port, protocol),
    KEY idx_alloc_node (node_id),
    CONSTRAINT fk_alloc_node FOREIGN KEY (node_id) REFERENCES nodes (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE servers (
    id CHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    template_id CHAR(36) NOT NULL,
    node_id CHAR(36) NULL,
    allocation_id CHAR(36) NULL,
    status VARCHAR(32) NOT NULL,
    environment_json JSON NOT NULL,
    memory_mb INT NOT NULL,
    cpu_shares INT NOT NULL,
    container_name VARCHAR(128) NULL,
    container_id VARCHAR(128) NULL,
    last_error TEXT NULL,
    files_json JSON NULL,
    created_by CHAR(36) NOT NULL,
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_servers_node (node_id),
    KEY idx_servers_status (status),
    CONSTRAINT fk_servers_template FOREIGN KEY (template_id) REFERENCES templates (id),
    CONSTRAINT fk_servers_node FOREIGN KEY (node_id) REFERENCES nodes (id),
    CONSTRAINT fk_servers_created_by FOREIGN KEY (created_by) REFERENCES users (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE jobs (
    id CHAR(36) NOT NULL,
    node_id CHAR(36) NOT NULL,
    server_id CHAR(36) NULL,
    kind VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    payload_json JSON NOT NULL,
    result_json JSON NULL,
    dispatched_at DATETIME(3) NULL,
    completed_at DATETIME(3) NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_jobs_node_status (node_id, status),
    KEY idx_jobs_server (server_id),
    CONSTRAINT fk_jobs_node FOREIGN KEY (node_id) REFERENCES nodes (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE server_logs (
    id BIGINT NOT NULL AUTO_INCREMENT,
    server_id CHAR(36) NOT NULL,
    node_id CHAR(36) NULL,
    stream VARCHAR(16) NOT NULL,
    chunk TEXT NOT NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_logs_server (server_id, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE backups (
    id CHAR(36) NOT NULL,
    server_id CHAR(36) NOT NULL,
    node_id CHAR(36) NOT NULL,
    status VARCHAR(32) NOT NULL,
    archive_path VARCHAR(512) NULL,
    size_bytes BIGINT NULL,
    error TEXT NULL,
    created_at DATETIME(3) NOT NULL,
    completed_at DATETIME(3) NULL,
    PRIMARY KEY (id),
    KEY idx_backups_server (server_id),
    CONSTRAINT fk_backups_server FOREIGN KEY (server_id) REFERENCES servers (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE schedules (
    id CHAR(36) NOT NULL,
    server_id CHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    interval_seconds INT NOT NULL,
    action VARCHAR(32) NOT NULL,
    enabled TINYINT(1) NOT NULL DEFAULT 1,
    last_run_at DATETIME(3) NULL,
    next_run_at DATETIME(3) NOT NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_schedules_next (enabled, next_run_at),
    CONSTRAINT fk_schedules_server FOREIGN KEY (server_id) REFERENCES servers (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE notifications (
    id CHAR(36) NOT NULL,
    user_id CHAR(36) NULL,
    kind VARCHAR(32) NOT NULL,
    title VARCHAR(255) NOT NULL,
    body TEXT NOT NULL,
    read_at DATETIME(3) NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_notifications_user (user_id, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
