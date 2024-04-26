use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

/// big box, rounded edges
pub fn render(ui: &mut egui::Ui, _pipe: &mut UIPipe<Config>, _ui_state: &mut UIState) {
    ui.label("it works");
}
