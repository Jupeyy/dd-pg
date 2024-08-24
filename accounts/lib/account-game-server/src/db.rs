use sqlx::any::AnyStatement;

/// Shared data for a db connection
pub struct DbConnectionShared {
    /// Prepared statement for
    /// [`crate::auto_login::queries::RegisterUser`]
    pub register_user_statement: AnyStatement<'static>,
}
