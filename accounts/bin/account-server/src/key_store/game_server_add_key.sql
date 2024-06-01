INSERT INTO 
    account_game_server_key 
        ( 
            account_id, 
            encrypted_key_pair, 
            public_key, 
            create_time 
        ) 
    SELECT 
        session.account_id, 
        ?, 
        ?, 
        CURRENT_TIMESTAMP() 
    FROM 
        session, 
        account 
    WHERE 
        session.pub_key = ? AND 
        session.hw_id = ? AND 
        session.account_id = account.id AND 
        account.verified = true AND 
        account.verified_game_server = true 
ON DUPLICATE KEY UPDATE 
    account_game_server_key.encrypted_key_pair = ?, 
    account_game_server_key.public_key = ?, 
    account_game_server_key.create_time = CURRENT_TIMESTAMP() 
;
