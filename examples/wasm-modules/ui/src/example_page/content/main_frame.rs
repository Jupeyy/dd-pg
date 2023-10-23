use api::graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<()>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
) {
    ui.label("it works");
}
