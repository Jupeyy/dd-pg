DELETE FROM
    login_tokens
WHERE
    login_tokens.valid_until <= UTC_TIMESTAMP();
