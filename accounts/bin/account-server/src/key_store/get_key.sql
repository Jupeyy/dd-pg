SELECT 
    IFNULL ( 
        ( 
            SELECT 
                account_keys.encrypted_key_pair 
            FROM 
                account_keys, 
                session 
            WHERE 
                account_keys.account_id = session.account_id AND 
                account_keys.game_server_group_account_id = ( 
                        SELECT 
                            account.id 
                        FROM 
                            account, 
                            account_game_server_key 
                        WHERE 
                            account_game_server_key.public_key = ? AND 
                            account.id = account_game_server_key.account_id 
                        LIMIT 1 
                    ) AND 
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
