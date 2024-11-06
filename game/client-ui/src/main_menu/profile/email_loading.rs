use egui::Spinner;

use crate::main_menu::user_data::{ProfileState, ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(
        ui,
        match tasks.state {
            ProfileState::EmailLoggingIn(_) => "Logging in by email",
            ProfileState::EmailLinkCredential(_) => "Linking email",
            ProfileState::EmailUnlinkCredential(_) => "Unlinking email",
            ProfileState::EmailLogoutAll(_) => "Logging out all by email",
            ProfileState::EmailDelete(_) => "Delete by email",
            ProfileState::EmailAccountToken { .. } => "Waiting for code",
            ProfileState::EmailCredentialAuthToken { .. } => "Waiting for code",
            _ => "not implemented",
        },
        tasks,
    );

    ui.add(Spinner::new());
}
