use ui_wasm_manager::{UIRenderCallbackFunc, UIWinitWrapper};

pub struct Error404Page {}

impl Error404Page {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc for Error404Page {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<UIWinitWrapper>,
        ui_state: &mut ui_base::types::UIState<UIWinitWrapper>,
        graphics: &mut graphics::graphics::Graphics,
        fs: &std::sync::Arc<base_fs::filesys::FileSystem>,
        io_batcher: &std::sync::Arc<std::sync::Mutex<base_fs::io_batcher::TokIOBatcher>>,
    ) {
        ui.label("Error 404: not found");
        if ui.button("return").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "", "");
        }
    }

    fn destroy(self: Box<Self>, graphics: &mut graphics::graphics::Graphics) {}
}
