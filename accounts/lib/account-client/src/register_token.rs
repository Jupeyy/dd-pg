use accounts_base::account_server::{account_id::AccountId, register_token::RegisterToken};

use crate::{
    errors::HttpLikeError,
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// Get the account id of the register token
/// from the account server.
pub async fn get_account_id_of_register_token(
    io: &dyn Io,
    register_token: RegisterToken,
) -> anyhow::Result<AccountId, HttpLikeError> {
    get_account_id_of_register_token_impl(io.into(), register_token).await
}

async fn get_account_id_of_register_token_impl(
    io: IoSafe<'_>,
    register_token: RegisterToken,
) -> anyhow::Result<AccountId, HttpLikeError> {
    io.account_id_of_register_token(register_token).await
}
