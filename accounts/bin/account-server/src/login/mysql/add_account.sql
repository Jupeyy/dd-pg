INSERT INTO
    account (
        email,
        steamid,
        create_time
    )
VALUES
    (
        ?,
        ?,
        UTC_TIMESTAMP()
    );
