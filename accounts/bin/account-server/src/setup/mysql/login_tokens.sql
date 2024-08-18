CREATE TABLE login_tokens (
    token BINARY(32) NOT NULL,
    valid_until DATETIME NOT NULL,
    email VARCHAR(255),
    steamid VARCHAR(255),
    PRIMARY KEY(token) USING HASH,
    INDEX(email) USING HASH,
    INDEX(steamid) USING HASH
) ENGINE = MEMORY;
