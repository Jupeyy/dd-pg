use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;
use ui_wasm_manager::UIWinitWrapper;

pub struct LoadingPage {}

impl LoadingPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend> for LoadingPage {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _pipe: &mut ui_base::types::UIPipe<UIWinitWrapper>,
        _ui_state: &mut ui_base::types::UIState<UIWinitWrapper>,
        _graphics: &mut Graphics,
    ) {
        ui.label("Loading page...");
    }
}
