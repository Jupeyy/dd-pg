UPDATE 
    account 
INNER JOIN 
    verify_tokens 
ON 
    ( 
        verify_tokens.account_id = account.id 
    ) 
SET 
    account.verified = true, 
    verify_tokens.valid_until = CURRENT_TIMESTAMP() 
WHERE 
    verify_tokens.token = ? AND 
    verify_tokens.valid_until > CURRENT_TIMESTAMP() 
;
