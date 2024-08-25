SELECT
    account_tokens.account_id
FROM
    account_tokens
WHERE
    account_tokens.token = ?
    AND account_tokens.valid_until > UTC_TIMESTAMP();
