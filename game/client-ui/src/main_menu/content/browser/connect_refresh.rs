use egui::Color32;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::{
    components::menu_top_button::text_icon,
    style::default_style,
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

/// connect & refresh button
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
    ui.horizontal(|ui| {
        let mut button_style = default_style();
        button_style.visuals.widgets.inactive.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.noninteractive.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.active.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.hovered.weak_bg_fill = Color32::DARK_GREEN;
        ui.set_style(button_style);
        let text = text_icon(ui, "\u{f2f6}");
        ui.button(text);
    });
    let text = text_icon(ui, "\u{f2f9}");
    ui.button(text);
}
