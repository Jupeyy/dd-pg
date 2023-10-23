use egui::{epaint::RectShape, Color32, Layout, RichText};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::user_data::UserData;

/// can contain various information
/// depends on the modification
/// map name, scorelimit, round
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
    ui.painter().add(RectShape::filled(
        ui.available_rect_before_wrap(),
        0.0,
        Color32::DARK_GRAY,
    ));
    const FONT_SIZE: f32 = 8.0;
    ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add_space(5.0);
        ui.label(RichText::new("Score limit: 500").size(FONT_SIZE));
    });
}
