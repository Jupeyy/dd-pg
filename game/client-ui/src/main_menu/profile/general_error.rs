use egui::ScrollArea;

use crate::main_menu::user_data::{ProfileState, ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(ui, "An error occurred", tasks);

    if let ProfileState::Err(err) = &tasks.state {
        ui.label("The following errors occurred:");
        ScrollArea::vertical().show(ui, |ui| {
            ui.label(err);
        });
        if ui.button("Try again").clicked() {
            tasks.state = ProfileState::Overview;
        }
    }
}
