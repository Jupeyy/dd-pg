use std::sync::Arc;

use crate::main_menu::{
    profiles_interface::{LinkedCredential, ProfilesInterface},
    user_data::{AccountOperation, ProfileState, ProfileTasks},
};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, accounts: &Arc<dyn ProfilesInterface>, tasks: &mut ProfileTasks) {
    back_bar(ui, "Logout all", tasks);

    if let ProfileState::LogoutAllPrepare { profile_name, info } = &tasks.state {
        let accounts = accounts.clone();
        if info.credentials.iter().any(|c| {
            if let LinkedCredential::Steam(steam_id) = c {
                accounts.steam_id64() == *steam_id
            } else {
                false
            }
        }) {
            tasks.state = ProfileState::SteamAccountTokenPrepare(AccountOperation::LogoutAll {
                profile_name: profile_name.clone(),
            });
        } else {
            tasks.state = ProfileState::EmailAccountTokenPrepare(AccountOperation::LogoutAll {
                profile_name: profile_name.clone(),
            });
        }
    }
}
