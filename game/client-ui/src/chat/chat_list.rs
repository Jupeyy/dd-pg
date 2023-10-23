use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

/// frame for the chat entries
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<()>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
}
