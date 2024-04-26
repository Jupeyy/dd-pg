use egui_extras::TableBody;

use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// server list frame (scrollable)
pub fn render(body: TableBody<'_>, pipe: &mut UIPipe<UserData>, ui_state: &mut UIState) {
    body.rows(25.0, 100, |row| {
        let row_index = row.index();
        super::entry::render(row, row_index, pipe, ui_state);
    });
}
