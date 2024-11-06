use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
    main_frame_only: bool,
) {
    if cur_page.is_empty()
        || cur_page == "Internet"
        || cur_page == "LAN"
        || cur_page == "Favorites"
        || cur_page == "ddnet"
    {
        super::browser::main_frame::render(ui, pipe, ui_state, cur_page, main_frame_only);
    } else if cur_page == "Communities" {
    }
}
