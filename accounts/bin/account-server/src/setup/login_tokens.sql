CREATE TABLE login_tokens (
    token BINARY(32) NOT NULL,
    valid_until DATETIME NOT NULL,
    email VARCHAR(255),
    steamid VARCHAR(255),
    PRIMARY KEY(token),
    UNIQUE KEY(email),
    UNIQUE KEY(steamid)
) ENGINE = MEMORY;
