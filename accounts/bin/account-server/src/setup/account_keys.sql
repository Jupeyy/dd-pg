CREATE TABLE account_keys (
    account_id BIGINT UNSIGNED NOT NULL,
    game_server_group_account_id BIGINT UNSIGNED NOT NULL,
    encrypted_key_pair VARBINARY(1024) NOT NULL,
    create_time DATETIME NOT NULL,
    PRIMARY KEY(account_id, game_server_group_account_id),
    FOREIGN KEY(account_id) REFERENCES account(id),
    FOREIGN KEY(game_server_group_account_id) REFERENCES account(id)
);
