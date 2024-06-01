use egui_extras::TableRow;

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// table header
pub fn render(
    header: &mut TableRow<'_, '_>,
    _pipe: &mut UiRenderPipe<UserData>,
    _ui_state: &mut UiState,
) {
    header.col(|ui| {
        ui.strong("");
    });
    header.col(|ui| {
        ui.strong("Name");
    });
    header.col(|ui| {
        ui.strong("Type");
    });
    header.col(|ui| {
        ui.strong("Map");
    });
    header.col(|ui| {
        ui.strong("Players");
    });
    header.col(|ui| {
        ui.strong("Ping");
    });
}
