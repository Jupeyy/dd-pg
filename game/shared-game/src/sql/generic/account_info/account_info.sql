SELECT
    user.id,
    user.name,
    user.create_time
FROM
    user
WHERE
    user.account_id = ?;
