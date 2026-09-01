-- Canonical schema version 1 for 0.0.1-alpha.1
-- Timestamps are stored as UTC DATETIME(3). Identifiers are UUIDv7 text.

CREATE TABLE users (
    id CHAR(36) NOT NULL,
    email VARCHAR(320) NOT NULL,
    display_name VARCHAR(128) NOT NULL,
    role VARCHAR(32) NOT NULL,
    totp_secret_encrypted TEXT NULL,
    totp_enabled TINYINT(1) NOT NULL DEFAULT 0,
    recovery_hashes_json JSON NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    disabled_at DATETIME(3) NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_users_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE credentials (
    id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    kind VARCHAR(32) NOT NULL,
    secret_hash VARCHAR(255) NOT NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_credentials_user (user_id),
    CONSTRAINT fk_credentials_user FOREIGN KEY (user_id) REFERENCES users (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE sessions (
    id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    token_hash CHAR(64) NOT NULL,
    csrf_token_hash CHAR(64) NOT NULL,
    refresh_token_hash CHAR(64) NULL,
    user_agent VARCHAR(512) NULL,
    ip VARCHAR(64) NULL,
    expires_at DATETIME(3) NOT NULL,
    refresh_expires_at DATETIME(3) NULL,
    revoked_at DATETIME(3) NULL,
    created_at DATETIME(3) NOT NULL,
    last_used_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_sessions_token (token_hash),
    UNIQUE KEY uq_sessions_refresh (refresh_token_hash),
    KEY idx_sessions_user (user_id),
    CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE invitations (
    id CHAR(36) NOT NULL,
    email VARCHAR(320) NOT NULL,
    role VARCHAR(32) NOT NULL,
    token_hash CHAR(64) NOT NULL,
    invited_by CHAR(36) NOT NULL,
    expires_at DATETIME(3) NOT NULL,
    accepted_at DATETIME(3) NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_invitations_token (token_hash),
    KEY idx_invitations_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE nodes (
    id CHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    hostname VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL,
    agent_version VARCHAR(64) NULL,
    protocol_version INT NOT NULL,
    architecture VARCHAR(32) NULL,
    operating_system VARCHAR(64) NULL,
    labels_json JSON NOT NULL,
    docker_state VARCHAR(32) NOT NULL,
    docker_engine_version VARCHAR(64) NULL,
    docker_error TEXT NULL,
    last_heartbeat_at DATETIME(3) NULL,
    maintenance TINYINT(1) NOT NULL DEFAULT 0,
    enrolled_at DATETIME(3) NOT NULL,
    certificate_fingerprint CHAR(64) NOT NULL,
    token_hash CHAR(64) NOT NULL,
    cpu_cores INT NULL,
    memory_bytes BIGINT NULL,
    disk_bytes BIGINT NULL,
    disk_available_bytes BIGINT NULL,
    health_message VARCHAR(512) NULL,
    workload_count INT NOT NULL DEFAULT 0,
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_nodes_token (token_hash),
    KEY idx_nodes_status (status),
    KEY idx_nodes_heartbeat (last_heartbeat_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE node_enrollment_tokens (
    id CHAR(36) NOT NULL,
    token_hash CHAR(64) NOT NULL,
    label VARCHAR(128) NULL,
    created_by CHAR(36) NOT NULL,
    expires_at DATETIME(3) NOT NULL,
    consumed_at DATETIME(3) NULL,
    consumed_by_node CHAR(36) NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_enrollment_token (token_hash),
    KEY idx_enrollment_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE audit_events (
    id CHAR(36) NOT NULL,
    actor_user_id CHAR(36) NULL,
    actor_node_id CHAR(36) NULL,
    action VARCHAR(64) NOT NULL,
    resource_type VARCHAR(64) NOT NULL,
    resource_id CHAR(36) NULL,
    ip VARCHAR(64) NULL,
    request_id CHAR(36) NULL,
    details_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_audit_created (created_at),
    KEY idx_audit_action (action),
    KEY idx_audit_actor (actor_user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE idempotency_keys (
    id CHAR(36) NOT NULL,
    actor_hash CHAR(64) NOT NULL,
    key_hash CHAR(64) NOT NULL,
    method VARCHAR(16) NOT NULL,
    path VARCHAR(255) NOT NULL,
    request_hash CHAR(64) NOT NULL,
    status INT NOT NULL,
    response_body MEDIUMBLOB NOT NULL,
    created_at DATETIME(3) NOT NULL,
    expires_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_idem (actor_hash, key_hash, method, path),
    KEY idx_idem_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE platform_settings (
    k VARCHAR(64) NOT NULL,
    v_json JSON NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    PRIMARY KEY (k)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE update_history (
    id CHAR(36) NOT NULL,
    component VARCHAR(64) NOT NULL,
    from_version VARCHAR(64) NULL,
    to_version VARCHAR(64) NOT NULL,
    channel VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    details_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_update_created (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
