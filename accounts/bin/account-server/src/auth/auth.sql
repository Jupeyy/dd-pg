SELECT 
    session.account_id, 
    session.secret, 
    account.verified 
FROM 
    account, 
    session 
WHERE 
    session.pub_key = ? AND 
    session.hw_id = ? AND 
    account.id = session.account_id 
;
