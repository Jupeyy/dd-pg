use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    main_frame_only: bool,
) {
    let dummy_str = String::new();
    let cur_page = pipe.config.ui.path.query.get("main").unwrap_or(&dummy_str);
    if cur_page.is_empty()
        || cur_page == "Internet"
        || cur_page == "LAN"
        || cur_page == "Favorites"
        || cur_page == "DDNet"
        || cur_page == "Community"
    {
        super::browser::main_frame::render(ui, pipe, ui_state, graphics, main_frame_only);
    }
}
