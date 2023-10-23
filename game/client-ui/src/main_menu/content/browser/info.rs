use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// simply a label
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
    ui.label("TODO: info");
}
