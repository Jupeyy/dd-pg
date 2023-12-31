use egui_extras::TableRow;
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// table header
pub fn render(
    header: &mut TableRow<'_, '_>,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
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
