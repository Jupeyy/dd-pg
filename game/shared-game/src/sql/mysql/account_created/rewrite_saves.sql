UPDATE
    user_save
SET
    user_save.user_id = ?
WHERE
    user_save.user_hash = ?
    AND user_save.user_id = NULL;
