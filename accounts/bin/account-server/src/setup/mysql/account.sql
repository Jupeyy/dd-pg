CREATE TABLE account (
    id BIGINT NOT NULL AUTO_INCREMENT,
    email VARCHAR(255),
    steamid VARCHAR(255),
    -- UTC timestamp! (UTC_TIMESTAMP())
    create_time DATETIME NOT NULL,
    PRIMARY KEY(id),
    UNIQUE KEY(email),
    UNIQUE KEY(steamid)
);
