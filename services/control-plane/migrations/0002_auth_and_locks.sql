-- Schema version 2: setup singleton lock, pending TOTP, unique node fingerprint.

ALTER TABLE users
    ADD COLUMN totp_pending_encrypted TEXT NULL AFTER totp_secret_encrypted;

ALTER TABLE nodes
    ADD UNIQUE KEY uq_nodes_fingerprint (certificate_fingerprint);

CREATE TABLE singleton_locks (
    lock_name CHAR(32) NOT NULL,
    PRIMARY KEY (lock_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO singleton_locks (lock_name) VALUES ('setup');
