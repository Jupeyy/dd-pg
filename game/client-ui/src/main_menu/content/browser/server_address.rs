use egui::{FontId, TextFormat};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::{
    components::{clearable_edit_field::clearable_edit_field, menu_top_button::text_icon},
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

/// server address input field
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
    ui.horizontal(|ui| {
        let mut text = text_icon(ui, "\u{f233}");
        text.append(
            "address:",
            4.0,
            TextFormat::simple(FontId::default(), ui.style().visuals.text_color()),
        );
        ui.label(text);
    });
    clearable_edit_field(ui, &mut pipe.user_data.browser_data.cur_address);
}
