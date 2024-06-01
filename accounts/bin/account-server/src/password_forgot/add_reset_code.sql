INSERT INTO 
    reset_codes 
        (
            account_id, 
            token, 
            valid_until 
        ) 
    VALUES 
        (
            ?, 
            ?, 
            DATE_ADD(CURRENT_TIMESTAMP(), INTERVAL 15 MINUTE) 
        ) 
;
