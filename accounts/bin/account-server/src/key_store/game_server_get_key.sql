SELECT 
    IFNULL ( 
        ( 
            SELECT 
                account_game_server_key.encrypted_key_pair 
            FROM 
                account_game_server_key, 
                session 
            WHERE 
                account_game_server_key.account_id = session.account_id AND 
                session.pub_key = ? AND 
                session.hw_id = ? 
            LIMIT 1 
        ), 
        ( 
            SELECT 
                NULL AS encrypted_key_pair 
            FROM 
                session 
            WHERE 
                session.pub_key = ? AND 
                session.hw_id = ? 
            LIMIT 1 
        ) 
    )
;
