CREATE TABLE account_game_server_key (
    account_id BIGINT UNSIGNED NOT NULL,
    encrypted_key_pair VARBINARY(1024) NOT NULL,
    public_key BINARY(32) NOT NULL,
    create_time DATETIME NOT NULL,
    PRIMARY KEY(account_id),
    FOREIGN KEY(account_id) REFERENCES account(id),
    UNIQUE KEY(public_key)
);
