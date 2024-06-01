UPDATE 
    account 
SET 
    account.verified_game_server = true 
WHERE 
    account.id = ? 
;
