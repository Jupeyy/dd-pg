use std::collections::HashMap;

use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ProfileData {
    pub name: String,
}

#[async_trait]
pub trait ProfilesInterface: Sync + Send {
    /// requests a login token by email for a new session
    async fn login_email_token(&self, email: email_address::EmailAddress) -> anyhow::Result<()>;
    /// do the login process for an email using a token
    async fn login_email(
        &self,
        email: email_address::EmailAddress,
        token_b64: String,
    ) -> anyhow::Result<()>;
    /// user related interactions can be:
    /// - server list reload
    /// Which allows the implementation to fetch new certificates or similar tasks.
    async fn user_interaction(&self) -> anyhow::Result<()>;

    /// Currently loaded profiles
    fn profiles(&self) -> (HashMap<String, ProfileData>, String);
}
