use crate::main_menu::user_data::{CredentialAuthOperation, ProfileState, ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(ui, "Unlink email", tasks);

    if let ProfileState::UnlinkEmailPrepare { profile_name, .. } = &tasks.state {
        tasks.state = ProfileState::EmailCredentialAuthTokenPrepare(
            CredentialAuthOperation::UnlinkCredential {
                profile_name: profile_name.clone(),
            },
        );
    }
}
