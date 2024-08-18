use sqlx::any::AnyStatement;

/// Shared data for a db connection
pub struct DbConnectionShared {
    pub login_token_email_statement: AnyStatement<'static>,
    pub login_token_qry_statement: AnyStatement<'static>,
    pub invalidate_login_token_statement: AnyStatement<'static>,
    pub try_create_account_statement: AnyStatement<'static>,
    pub login_qry_statement: AnyStatement<'static>,
    pub create_session_statement: AnyStatement<'static>,
    pub logout_statement: AnyStatement<'static>,
    pub auth_attempt_statement: AnyStatement<'static>,
    pub account_token_email_statement: AnyStatement<'static>,
    pub account_token_qry_statement: AnyStatement<'static>,
    pub invalidate_account_token_statement: AnyStatement<'static>,
    pub remove_sessions_statement: AnyStatement<'static>,
    pub remove_account_statement: AnyStatement<'static>,
    pub add_cert_statement: AnyStatement<'static>,
    pub get_certs_statement: AnyStatement<'static>,
    pub cleanup_login_tokens_statement: AnyStatement<'static>,
    pub cleanup_account_tokens_statement: AnyStatement<'static>,
    pub cleanup_certs_statement: AnyStatement<'static>,
}
