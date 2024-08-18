SELECT
    account.id
FROM
    account
WHERE
    (
        account.email IS NOT NULL
        AND account.email = ?
    )
    OR (
        account.steamid IS NOT NULL
        AND account.steamid = ?
    );
