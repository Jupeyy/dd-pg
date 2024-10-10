use crate::main_menu::user_data::{CredentialAuthOperation, ProfileState, ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(ui, "Unlink steam", tasks);

    if let ProfileState::UnlinkSteamPrepare { profile_name, .. } = &tasks.state {
        tasks.state = ProfileState::SteamCredentialAuthTokenPrepare(
            CredentialAuthOperation::UnlinkCredential {
                profile_name: profile_name.clone(),
            },
        );
    }
}
