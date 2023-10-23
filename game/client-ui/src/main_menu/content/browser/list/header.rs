use egui_extras::TableRow;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// table header
pub fn render<B: GraphicsBackendInterface>(
    header: &mut TableRow<'_, '_>,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
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
