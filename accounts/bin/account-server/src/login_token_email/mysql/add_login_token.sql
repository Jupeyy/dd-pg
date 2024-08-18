INSERT INTO
    login_tokens (
        token,
        valid_until,
        email
    )
VALUES
    (
        ?,
        DATE_ADD(UTC_TIMESTAMP(), INTERVAL 15 MINUTE),
        ?
    );
