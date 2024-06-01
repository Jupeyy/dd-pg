INSERT INTO 
    account 
        (
            email, 
            password, 
            salt, 
            encrypted_main_secret,
            create_time
        ) 
    VALUES 
        (
            ?, 
            ?, 
            ?, 
            ?,
            CURRENT_TIMESTAMP()
        ) 
;
