SELECT 
    account.id, 
    account.password, 
    account.salt, 
    account.encrypted_main_secret 
FROM 
    account 
WHERE 
    account.email = ? 
;
