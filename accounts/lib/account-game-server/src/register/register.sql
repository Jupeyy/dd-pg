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
    user.user_id = ?, 
    user.account_id = IF (user.account_id IS NULL, ?, user.account_id) 
;
