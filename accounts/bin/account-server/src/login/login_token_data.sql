SELECT 
    login_tokens.email, 
    login_tokens.steamid 
FROM 
    login_tokens 
WHERE 
    login_tokens.token = ? 
;
