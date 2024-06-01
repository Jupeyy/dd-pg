use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use ui_base::types::{UiRenderPipe, UiState};

/// big box, rounded edges
pub fn render(ui: &mut egui::Ui, _pipe: &mut UiRenderPipe<Config>, _ui_state: &mut UiState) {
    ui.label("it works");
}
