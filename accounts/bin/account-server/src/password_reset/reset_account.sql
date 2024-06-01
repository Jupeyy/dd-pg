UPDATE 
    account 
INNER JOIN 
    reset_codes 
ON 
    ( 
        reset_codes.account_id = account.id 
    ) 
SET 
    account.password = ?, 
    account.salt = ?, 
    account.encrypted_main_secret = ?, 
    reset_codes.valid_until = CURRENT_TIMESTAMP() 
WHERE 
    reset_codes.token = ? AND 
    reset_codes.valid_until > CURRENT_TIMESTAMP() 
;
