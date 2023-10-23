use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct Error404Page {}

impl Error404Page {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<(), GraphicsBackend> for Error404Page {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<()>,
        ui_state: &mut ui_base::types::UIState,
        graphics: &mut graphics::graphics::GraphicsBase<GraphicsBackend>,
    ) {
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<()>,
        _ui_state: &mut ui_base::types::UIState,
        _graphics: &mut Graphics,
    ) {
        ui.label("Error 404: not found");
        if ui.button("return").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "", "");
        }
    }
}
