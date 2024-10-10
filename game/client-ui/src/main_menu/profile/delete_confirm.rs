use crate::main_menu::user_data::{ProfileState, ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(ui, "Delete account", tasks);

    if let ProfileState::DeleteConfirm { profile_name, info } = &tasks.state {
        let profile_name = profile_name.clone();
        let info = info.clone();
        ui.label("Are you sure you want to proceed?\nDeleting an account cannot be undone.");

        ui.horizontal(|ui| {
            if ui.button("Proceed & delete account").clicked() {
                tasks.state = ProfileState::DeletePrepare { profile_name, info };
            }
            if ui.button("Cancel").clicked() {
                tasks.state = ProfileState::Overview;
            }
        });
    }
}
