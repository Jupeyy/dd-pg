use async_trait::async_trait;

#[async_trait]
pub trait ProfilesInterface: Sync + Send {
    /// logs in a new user and adds it to the profiles
    async fn login(&self, email: email_address::EmailAddress, password: &str)
        -> anyhow::Result<()>;

    // registers a new user and adds it to the profiles on success
    async fn register(
        &self,
        email: email_address::EmailAddress,
        password: &str,
    ) -> anyhow::Result<()>;

    /// Auth the current active profile
    async fn try_auth_current_active(&self) -> anyhow::Result<()>;

    /// Currently loaded profiles
    fn profiles(&self) -> (Vec<String>, String);
}
