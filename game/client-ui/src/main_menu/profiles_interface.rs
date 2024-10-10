use std::collections::HashMap;

use async_trait::async_trait;
use game_interface::types::player_info::AccountId;
pub use url::Url;

#[derive(Debug, Clone)]
pub struct ProfileData {
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum LinkedCredential {
    Email(String),
    Steam(i64),
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub account_id: AccountId,
    pub creation_date: String,
    pub credentials: Vec<LinkedCredential>,
}

#[derive(Debug)]
pub enum CredentialAuthTokenError {
    WebValidationProcessNeeded {
        /// The url the client has to visit in order to continue
        url: Url,
    },
    Other(anyhow::Error),
}

#[derive(Debug)]
pub enum AccountTokenError {
    WebValidationProcessNeeded {
        /// The url the client has to visit in order to continue
        url: Url,
    },
    Other(anyhow::Error),
}

#[derive(Debug)]
pub enum CredentialAuthTokenOperation {
    /// Login using these credentials.
    Login,
    /// Link the credential to an account
    /// (e.g. email or steam).
    LinkCredential,
    /// Unlink the credential from its account
    /// (e.g. email or steam).
    /// If the credential is the last bound to
    /// the account this operation will fail and
    /// [`super::account_token::AccountTokenOperation::Delete`]
    /// should be used instead.
    UnlinkCredential,
}

#[derive(Debug)]
pub enum AccountTokenOperation {
    /// Logout all sessions at once.
    LogoutAll,
    /// Link another credential to this account
    /// (e.g. email or steam).
    LinkCredential,
    /// Delete the account.
    Delete,
}

#[async_trait]
pub trait ProfilesInterface: Sync + Send {
    /// Whether steam runtime is supported.
    fn supports_steam(&self) -> bool;

    /// Returns the steam id of the user (if steam runtime is supported)
    fn steam_id64(&self) -> i64;

    /// requests a credential auth token by email for a new session
    async fn credential_auth_email_token(
        &self,
        op: CredentialAuthTokenOperation,
        email: email_address::EmailAddress,
        secret_token: Option<String>,
    ) -> anyhow::Result<(), CredentialAuthTokenError>;
    /// requests a credential auth token by steam for a new session
    async fn credential_auth_steam_token(
        &self,
        op: CredentialAuthTokenOperation,
        secret_token: Option<String>,
    ) -> anyhow::Result<String, CredentialAuthTokenError>;

    /// requests an account token by email for a new session
    async fn account_email_token(
        &self,
        op: AccountTokenOperation,
        email: email_address::EmailAddress,
        secret_token: Option<String>,
    ) -> anyhow::Result<(), AccountTokenError>;
    /// requests an account token by steam for a new session
    async fn account_steam_token(
        &self,
        op: AccountTokenOperation,
        secret_token: Option<String>,
    ) -> anyhow::Result<String, AccountTokenError>;

    /// do the login process for an email using a token
    async fn login_email(
        &self,
        email: email_address::EmailAddress,
        token_hex: String,
    ) -> anyhow::Result<()>;
    /// do the login process for steam using a token
    async fn login_steam(&self, token_hex: String) -> anyhow::Result<()>;

    /// link a credential to an account
    async fn link_credential(
        &self,
        account_token_hex: String,
        credential_auth_token_hex: String,
        name: &str,
    ) -> anyhow::Result<()>;

    /// unlink a credential from an account
    async fn unlink_credential(
        &self,
        credential_auth_token_hex: String,
        name: &str,
    ) -> anyhow::Result<()>;

    /// Logs out the current session
    async fn logout(&self, name: &str) -> anyhow::Result<()>;

    /// Logs out all sessions except the current of an account
    async fn logout_all(&self, account_token_hex: String, name: &str) -> anyhow::Result<()>;

    /// Deletes an account
    async fn delete(&self, account_token_hex: String, name: &str) -> anyhow::Result<()>;

    /// user related interactions can be:
    /// - server list reload
    ///
    /// Which allows the implementation to fetch new certificates or similar tasks.
    async fn user_interaction(&self) -> anyhow::Result<()>;

    /// Fetches the account info for the given profile
    async fn account_info(&self, name: &str) -> anyhow::Result<AccountInfo>;

    /// Currently loaded profiles
    fn profiles(&self) -> (HashMap<String, ProfileData>, String);
    /// Set active profile
    async fn set_profile(&self, name: &str);
    /// Set's the display name of a profile
    async fn set_profile_display_name(&self, profile_name: &str, display_name: String);
}
