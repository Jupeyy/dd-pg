SELECT
    login_tokens.ty,
    login_tokens.identifier
FROM
    login_tokens
WHERE
    login_tokens.token = ?
    AND login_tokens.valid_until > UTC_TIMESTAMP();
