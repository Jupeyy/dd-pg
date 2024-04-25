use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

/// big square, rounded edges
pub fn render(ui: &mut egui::Ui, pipe: &mut UIPipe<Config>, ui_state: &mut UIState) {
    super::content::main_frame::render(ui, pipe, ui_state);
}
