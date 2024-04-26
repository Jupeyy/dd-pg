use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// simply a label
pub fn render(ui: &mut egui::Ui, _pipe: &mut UIPipe<UserData>, _ui_state: &mut UIState) {
    ui.label("TODO: info");
}
