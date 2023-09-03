use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;
use ui_wasm_manager::UIWinitWrapper;

pub struct Error404Page {}

impl Error404Page {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend> for Error404Page {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<UIWinitWrapper>,
        _ui_state: &mut ui_base::types::UIState<UIWinitWrapper>,
        _graphics: &mut Graphics,
    ) {
        ui.label("Error 404: not found");
        if ui.button("return").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "", "");
        }
    }
}
