use async_trait::async_trait;
use client_ui::main_menu::profiles_interface::ProfilesInterface;

pub struct Profiles;

#[async_trait]
impl ProfilesInterface for Profiles {
    async fn login(
        &self,
        email: email_address::EmailAddress,
        password: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    // registers a new user and adds it to the profiles on success
    async fn register(
        &self,
        email: email_address::EmailAddress,
        password: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Auth the current active profile
    async fn try_auth_current_active(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Currently loaded profiles
    fn profiles(&self) -> (Vec<String>, String) {
        Default::default()
    }
}
