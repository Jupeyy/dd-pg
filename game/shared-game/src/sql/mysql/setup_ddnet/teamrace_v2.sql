ALTER TABLE
    record_teamrace
ADD
    user_id BIGINT DEFAULT NULL,
ADD
    user_hash BINARY(32) DEFAULT NULL,
ADD
    UNIQUE KEY(user_id),
ADD
    UNIQUE KEY(user_hash);
