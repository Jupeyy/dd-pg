use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

/// big square, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<()>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
) {
    super::content::main_frame::render(ui, pipe, ui_state, graphics);
}
