CREATE TABLE user (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BINARY(32) NOT NULL,
    account_id BIGINT UNSIGNED,
    PRIMARY KEY(user_id),
    UNIQUE KEY(id),
    UNIQUE KEY(account_id)
);
