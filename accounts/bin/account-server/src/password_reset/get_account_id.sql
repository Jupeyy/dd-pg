SELECT 
    account.id 
FROM 
    account, 
    reset_codes 
WHERE 
    reset_codes.account_id = account.id AND
    reset_codes.token = ? AND 
    reset_codes.valid_until > CURRENT_TIMESTAMP() 
;
