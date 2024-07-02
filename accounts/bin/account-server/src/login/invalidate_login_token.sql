UPDATE 
    login_tokens 
SET 
    login_tokens.valid_until = CURRENT_TIMESTAMP()  
WHERE 
    login_tokens.token = ? 
;
