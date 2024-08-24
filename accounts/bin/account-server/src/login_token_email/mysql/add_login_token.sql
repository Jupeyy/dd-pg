INSERT INTO
    login_tokens (
        token,
        valid_until,
        ty,
        identifier
    )
VALUES
    (
        ?,
        DATE_ADD(UTC_TIMESTAMP(), INTERVAL 15 MINUTE),
        ?,
        ?
    );
