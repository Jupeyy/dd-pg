DELETE FROM
    SESSION
WHERE
    SESSION.pub_key = ?
    AND SESSION.hw_id = ?;
