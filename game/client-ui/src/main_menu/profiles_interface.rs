use async_trait::async_trait;

#[async_trait]
pub trait ProfilesInterface: Sync + Send {
    /// requests a login token by email for a new session
    async fn login_email_token(&self, email: email_address::EmailAddress) -> anyhow::Result<()>;

    /// Currently loaded profiles
    fn profiles(&self) -> (Vec<String>, String);
}
