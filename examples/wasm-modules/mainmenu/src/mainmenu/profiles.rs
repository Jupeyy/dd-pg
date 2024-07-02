use async_trait::async_trait;
use client_ui::main_menu::profiles_interface::ProfilesInterface;

pub struct Profiles;

#[async_trait]
impl ProfilesInterface for Profiles {
    async fn login_email_token(&self, email: email_address::EmailAddress) -> anyhow::Result<()> {
        Ok(())
    }

    /// Currently loaded profiles
    fn profiles(&self) -> (Vec<String>, String) {
        Default::default()
    }
}
