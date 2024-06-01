use std::{collections::HashMap, sync::Arc, time::Duration};

use accounts_base::account_server::{
    account_id::AccountId,
    register_token::{generate_register_token, RegisterToken},
};

pub const REGISTER_TOKEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Manager for register tokens.
/// Automatically removes them after a timeout hits.
#[derive(Debug, Default)]
pub struct RegisterTokens {
    active_register_tokens: Arc<parking_lot::Mutex<HashMap<RegisterToken, AccountId>>>,
}

impl RegisterTokens {
    /// Generates a new register token.
    /// Allowing operations with it for a specific time
    /// ([`REGISTER_TOKEN_TIMEOUT`]).
    pub fn gen_register_token_for(&self, account_id: AccountId) -> RegisterToken {
        let register_token = generate_register_token();
        let register_tokens = self.active_register_tokens.clone();
        // remove register token after some time
        tokio::spawn(async move {
            tokio::time::sleep(REGISTER_TOKEN_TIMEOUT).await;
            register_tokens.lock().remove(&register_token);
        });
        // add register token to active list
        self.active_register_tokens
            .lock()
            .insert(register_token, account_id);
        register_token
    }

    /// Checks if the register token exists, removes it from active register tokens
    /// and returns the account id if the register token existed.
    /// If this returns `Some`, that basically means the client
    /// has access to the given account id.
    pub fn try_consume_register_token(&self, register_token: RegisterToken) -> Option<AccountId> {
        self.active_register_tokens.lock().remove(&register_token)
    }
}
