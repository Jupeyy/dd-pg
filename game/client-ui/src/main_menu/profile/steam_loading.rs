use egui::Spinner;

use crate::main_menu::user_data::{ProfileState, ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(
        ui,
        match tasks.state {
            ProfileState::SteamLoggingIn(_) => "Logging in by steam",
            ProfileState::SteamLinkCredential(_) => "Linking steam",
            ProfileState::SteamUnlinkCredential(_) => "Unlinking steam",
            ProfileState::SteamLogoutAll(_) => "Logging out all by steam",
            ProfileState::SteamDelete(_) => "Delete by steam",
            ProfileState::SteamAccountToken { .. } => "Waiting for token",
            ProfileState::SteamCredentialAuthToken { .. } => "Waiting for token",
            _ => "not implemented",
        },
        tasks,
    );

    ui.add(Spinner::new());
}
