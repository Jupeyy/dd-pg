use egui_extras::TableRow;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// single server list entry
pub fn render<B: GraphicsBackendInterface>(
    mut row: TableRow<'_, '_>,
    row_index: usize,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
    row.col(|ui| {
        ui.label(format!("time"));
    });
    row.col(|ui| {
        ui.label(format!("{row_index}"));
    });
    row.col(|ui| {
        ui.label("flag");
    });
}
