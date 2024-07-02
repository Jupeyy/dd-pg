use sqlx::mysql::MySqlStatement;

/// Shared data for a MySQL connection
#[derive(Debug)]
pub struct MySqlConnectionShared {
    pub login_token_email_statement: MySqlStatement<'static>,
    pub login_token_qry_statement: MySqlStatement<'static>,
    pub invalidate_login_token_statement: MySqlStatement<'static>,
    pub try_create_account_statement: MySqlStatement<'static>,
    pub login_qry_statement: MySqlStatement<'static>,
    pub create_session_statement: MySqlStatement<'static>,
    pub auth_attempt_statement: MySqlStatement<'static>,
}
