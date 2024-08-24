CREATE TABLE account_tokens (
    account_id BIGINT NOT NULL,
    token BINARY(32) NOT NULL,
    valid_until DATETIME NOT NULL,
    FOREIGN KEY(account_id) REFERENCES account(id),
    PRIMARY KEY(token) USING HASH
) ENGINE = MEMORY;
