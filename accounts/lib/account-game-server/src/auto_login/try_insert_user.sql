INSERT INTO 
    user 
        ( 
            user_id, 
            account_id 
        ) 
    VALUES 
        ( 
            ?, 
            ? 
        ) 
ON DUPLICATE KEY 
UPDATE 
    user.user_id = IF ( 
        user.account_id IS NULL AND 
        ? IS NULL, 
        ?, 
        null 
    ), 
    user.account_id = ? 
;
