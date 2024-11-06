use std::sync::Arc;

use crate::main_menu::{
    profiles_interface::{LinkedCredential, ProfilesInterface},
    user_data::{AccountCredential, CredentialAuthOperation, ProfileState, ProfileTasks},
};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, accounts: &Arc<dyn ProfilesInterface>, tasks: &mut ProfileTasks) {
    back_bar(ui, "Link email", tasks);

    if let ProfileState::LinkEmailPrepare { profile_name, info } = &tasks.state {
        let accounts = accounts.clone();
        if info.credentials.iter().any(|c| {
            if let LinkedCredential::Steam(steam_id) = c {
                accounts.steam_id64() == *steam_id
            } else {
                false
            }
        }) {
            tasks.state = ProfileState::EmailCredentialAuthTokenPrepare(
                CredentialAuthOperation::LinkCredential {
                    profile_name: profile_name.clone(),
                    account_credential: AccountCredential::Steam,
                },
            );
        } else if info
            .credentials
            .iter()
            .any(|c| matches!(c, LinkedCredential::Email(_)))
        {
            tasks.state = ProfileState::SteamCredentialAuthTokenPrepare(
                CredentialAuthOperation::LinkCredential {
                    profile_name: profile_name.clone(),
                    account_credential: AccountCredential::Email,
                },
            );
        } else {
            // Logged into different steam acc
            tasks.state = ProfileState::Err(
                "The linked steam credential does not match the one of the current runtime"
                    .to_string(),
            );
        }
    }
}
