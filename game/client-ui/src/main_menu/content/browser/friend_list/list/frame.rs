use egui_extras::TableBody;

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// server list frame (scrollable)
pub fn render(body: TableBody<'_>, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    body.rows(25.0, 100, |row| {
        let row_index = row.index();
        super::entry::render(row, row_index, pipe, ui_state);
    });
}
