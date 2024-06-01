CREATE TABLE register_tokens (
    account_id BIGINT UNSIGNED NOT NULL,
    token BINARY(32) NOT NULL,
    valid_until DATETIME NOT NULL,
    FOREIGN KEY(account_id) REFERENCES account(id),
    UNIQUE KEY(token)
) ENGINE = MEMORY;
