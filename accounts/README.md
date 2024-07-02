Setup for tests:

```sql
CREATE USER 'ddnet-account-test'@localhost IDENTIFIED BY 'test';
CREATE DATABASE ddnet_account_test;
GRANT ALL PRIVILEGES ON ddnet_account_test.* TO 'ddnet-account-test'@localhost;
```


There has to be a smtp server running (for fake emails)
