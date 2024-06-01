INSERT INTO 
    account_keys 
        ( 
            account_id, 
            game_server_group_account_id, 
            encrypted_key_pair, 
            create_time 
        ) 
    SELECT 
        session.account_id, 
        ( 
            SELECT 
                account.id 
            FROM 
                account, 
                account_game_server_key 
            WHERE 
                account_game_server_key.public_key = ? AND 
                account.id = account_game_server_key.account_id 
            LIMIT 1 
        ), 
        ?, 
        CURRENT_TIMESTAMP() 
    FROM 
        session, 
        account 
    WHERE 
        session.pub_key = ? AND 
        session.hw_id = ? AND
        session.account_id = account.id AND 
        account.verified = true 
ON DUPLICATE KEY UPDATE 
    account_keys.encrypted_key_pair = ?, 
    account_keys.create_time = CURRENT_TIMESTAMP() 
;
