use egui::Spinner;

use crate::main_menu::user_data::ProfileTasks;

use super::back_bar::back_bar;

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks) {
    back_bar(ui, "Logging out", tasks);

    ui.add(Spinner::new());
}
