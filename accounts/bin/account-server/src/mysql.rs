use sqlx::mysql::MySqlStatement;

/// Shared data for a MySQL connection
#[derive(Debug)]
pub struct MySqlConnectionShared {
    pub register_statement: MySqlStatement<'static>,
    pub complete_register_statement: MySqlStatement<'static>,
    pub admin_verify_account_game_server_group_statement: MySqlStatement<'static>,
    pub add_verify_token: MySqlStatement<'static>,
    pub login_attempt_statement: MySqlStatement<'static>,
    pub create_session_statement: MySqlStatement<'static>,
    pub auth_attempt_statement: MySqlStatement<'static>,
    pub email_exists_statement: MySqlStatement<'static>,
    pub add_reset_code_statement: MySqlStatement<'static>,
    pub verify_reset_code_and_reset_account_statement: MySqlStatement<'static>,
    pub get_game_server_group_key_pair_statement: MySqlStatement<'static>,
    pub game_server_get_game_server_group_key_pair_statement: MySqlStatement<'static>,
    pub store_game_server_group_key_pair_statement: MySqlStatement<'static>,
    pub game_server_store_game_server_group_key_pair_statement: MySqlStatement<'static>,
    pub get_account_id_from_reset_code_statement: MySqlStatement<'static>,
    pub clear_client_keys_statement: MySqlStatement<'static>,
    pub clear_game_server_key_statement: MySqlStatement<'static>,
    pub clear_sessions_statement: MySqlStatement<'static>,
}
