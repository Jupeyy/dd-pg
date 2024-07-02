CREATE TABLE user (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BINARY(32),
    account_id BIGINT UNSIGNED,
    PRIMARY KEY(id),
    UNIQUE KEY(user_id),
    UNIQUE KEY(account_id)
);
