CREATE TABLE SESSION (
    account_id BIGINT NOT NULL,
    pub_key BINARY(32) NOT NULL,
    hw_id BINARY(32) NOT NULL,
    FOREIGN KEY(account_id) REFERENCES account(id),
    UNIQUE KEY(pub_key)
);
