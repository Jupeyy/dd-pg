SELECT 
    session.account_id, 
    account.create_time 
FROM 
    account, 
    session 
WHERE 
    session.pub_key = ? AND 
    session.hw_id = ? AND 
    account.id = session.account_id 
;
