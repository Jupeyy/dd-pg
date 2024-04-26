use egui_extras::TableRow;

use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// single server list entry
pub fn render(
    mut row: TableRow<'_, '_>,
    row_index: usize,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
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
