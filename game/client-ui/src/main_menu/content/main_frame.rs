use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
    main_frame_only: bool,
) {
    let dummy_str = String::new();
    let cur_page = pipe
        .config
        .ui
        .path
        .query
        .get("main")
        .unwrap_or(&dummy_str)
        .clone();
    if cur_page.is_empty()
        || cur_page == "Internet"
        || cur_page == "LAN"
        || cur_page == "Favorites"
        || cur_page == "DDNet"
        || cur_page == "Community"
    {
        super::browser::main_frame::render(
            ui,
            pipe,
            ui_state,
            graphics,
            &cur_page,
            main_frame_only,
        );
    }
}
