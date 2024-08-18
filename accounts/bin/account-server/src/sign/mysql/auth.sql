SELECT
    SESSION.account_id,
    account.create_time
FROM
    account,
    SESSION
WHERE
    SESSION.pub_key = ?
    AND SESSION.hw_id = ?
    AND account.id = SESSION.account_id;
