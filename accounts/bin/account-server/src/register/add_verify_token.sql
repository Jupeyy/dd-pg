INSERT INTO 
    verify_tokens 
        (
            account_id, 
            token, 
            valid_until 
        ) 
    VALUES 
        (
            LAST_INSERT_ID(), 
            ?, 
            DATE_ADD(CURRENT_TIMESTAMP(), INTERVAL 15 MINUTE) 
        ) 
;
