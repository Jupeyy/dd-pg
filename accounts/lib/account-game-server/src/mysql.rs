use sqlx::mysql::MySqlStatement;

/// Shared data for a MySQL connection
#[derive(Debug)]
pub struct MySqlConnectionShared {
    /// Prepared statement for
    /// [`crate::auto_login::queries::RegisterUser`]
    pub register_user_statement: MySqlStatement<'static>,
}
