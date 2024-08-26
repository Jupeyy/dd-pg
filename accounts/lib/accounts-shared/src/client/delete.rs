use anyhow::anyhow;
use base64::Engine;
use serde::{Deserialize, Serialize};

use super::account_token::AccountToken;

/// Represents the data required for a delete attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    /// An account token that is used to verify that the delete
    /// request is valid.
    pub account_token: AccountToken,
}

/// Represents the data required for a "delete sessions" attempt.
pub type DeleteSessionsRequest = DeleteRequest;

/// Prepares a delete request for the account server.
pub fn delete(account_token_b64: String) -> anyhow::Result<DeleteRequest> {
    let account_token = base64::prelude::BASE64_URL_SAFE.decode(account_token_b64)?;

    Ok(DeleteRequest {
        account_token: account_token
            .try_into()
            .map_err(|_| anyhow!("Invalid account token."))?,
    })
}

/// Prepares a delete sessions request for the account server.
pub fn delete_sessions(account_token_b64: String) -> anyhow::Result<DeleteSessionsRequest> {
    let account_token = base64::prelude::BASE64_URL_SAFE.decode(account_token_b64)?;

    Ok(DeleteSessionsRequest {
        account_token: account_token
            .try_into()
            .map_err(|_| anyhow!("Invalid account token."))?,
    })
}
