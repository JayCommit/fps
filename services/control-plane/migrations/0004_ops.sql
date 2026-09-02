-- Schema version 4: resource samples, crash-loop counters, file content cache.

CREATE TABLE resource_samples (
    id BIGINT NOT NULL AUTO_INCREMENT,
    node_id CHAR(36) NOT NULL,
    server_id CHAR(36) NULL,
    cpu_percent DOUBLE NULL,
    memory_bytes BIGINT NULL,
    disk_available_bytes BIGINT NULL,
    load_one FLOAT NULL,
    running TINYINT(1) NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_samples_node_time (node_id, created_at),
    KEY idx_samples_server_time (server_id, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

ALTER TABLE servers
    ADD COLUMN restart_count INT NOT NULL DEFAULT 0 AFTER last_error,
    ADD COLUMN consecutive_failures INT NOT NULL DEFAULT 0 AFTER restart_count,
    ADD COLUMN last_crash_at DATETIME(3) NULL AFTER consecutive_failures,
    ADD COLUMN last_file_json JSON NULL AFTER files_json;
