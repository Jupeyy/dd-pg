use egui_extras::TableBody;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// server list frame (scrollable)
pub fn render<B: GraphicsBackendInterface>(
    body: TableBody<'_>,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
) {
    body.rows(25.0, 100, |row_index, row| {
        super::entry::render(row, row_index, pipe, ui_state, graphics);
    });
}
