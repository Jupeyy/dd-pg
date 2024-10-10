use std::collections::HashMap;

use anyhow::anyhow;
use async_trait::async_trait;
use client_ui::main_menu::profiles_interface::{
    AccountInfo, AccountTokenError, AccountTokenOperation, CredentialAuthTokenError,
    CredentialAuthTokenOperation, ProfileData, ProfilesInterface,
};

pub struct Profiles;

#[async_trait]
impl ProfilesInterface for Profiles {
    fn supports_steam(&self) -> bool {
        true
    }

    fn steam_id64(&self) -> i64 {
        -1
    }

    async fn credential_auth_email_token(
        &self,
        _op: CredentialAuthTokenOperation,
        _email: email_address::EmailAddress,
        _secret_token: Option<String>,
    ) -> anyhow::Result<(), CredentialAuthTokenError> {
        Ok(())
    }

    async fn credential_auth_steam_token(
        &self,
        _op: CredentialAuthTokenOperation,
        _secret_token: Option<String>,
    ) -> anyhow::Result<String, CredentialAuthTokenError> {
        Ok("".to_string())
    }

    async fn account_email_token(
        &self,
        _op: AccountTokenOperation,
        _email: email_address::EmailAddress,
        _secret_token: Option<String>,
    ) -> anyhow::Result<(), AccountTokenError> {
        Ok(())
    }

    async fn account_steam_token(
        &self,
        _op: AccountTokenOperation,
        _secret_token: Option<String>,
    ) -> anyhow::Result<String, AccountTokenError> {
        Ok("".to_string())
    }

    async fn login_email(
        &self,
        _email: email_address::EmailAddress,
        _token_hex: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn login_steam(&self, _token_hex: String) -> anyhow::Result<()> {
        Ok(())
    }

    async fn link_credential(
        &self,
        _account_token_hex: String,
        _credential_auth_token_hex: String,
        _name: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn unlink_credential(
        &self,
        _credential_auth_token_hex: String,
        _name: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn logout(&self, _name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn logout_all(&self, _account_token_hex: String, _name: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn delete(&self, _account_token_hex: String, _name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn user_interaction(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn account_info(&self, _name: &str) -> anyhow::Result<AccountInfo> {
        Err(anyhow!("No account info fetched"))
    }

    /// Currently loaded profiles
    fn profiles(&self) -> (HashMap<String, ProfileData>, String) {
        Default::default()
    }

    async fn set_profile(&self, _name: &str) {}

    async fn set_profile_display_name(&self, _profile_name: &str, _display_name: String) {}
}
