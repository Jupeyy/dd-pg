use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use ui_base::types::{UiRenderPipe, UiState};

/// big square, rounded edges
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<Config>, ui_state: &mut UiState) {
    super::content::main_frame::render(ui, pipe, ui_state);
}
