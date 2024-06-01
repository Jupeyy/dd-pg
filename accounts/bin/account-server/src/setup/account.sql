CREATE TABLE account (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    email VARCHAR(255) NOT NULL,
    password BINARY(32) NOT NULL,
    salt VARCHAR(255) NOT NULL,
    encrypted_main_secret VARBINARY(1024) NOT NULL,
    verified BOOL NOT NULL DEFAULT false,
    verified_game_server BOOL NOT NULL DEFAULT false,
    create_time DATETIME NOT NULL,
    PRIMARY KEY(id),
    UNIQUE KEY(email)
);
