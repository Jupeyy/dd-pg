use ui_wasm_manager::{UIRenderCallbackFunc, UIWinitWrapper};

pub struct LoadingPage {}

impl LoadingPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc for LoadingPage {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<UIWinitWrapper>,
        ui_state: &mut ui_base::types::UIState<UIWinitWrapper>,
        graphics: &mut graphics::graphics::Graphics,
        fs: &std::sync::Arc<base_fs::filesys::FileSystem>,
        io_batcher: &std::sync::Arc<std::sync::Mutex<base_fs::io_batcher::TokIOBatcher>>,
    ) {
        ui.label("Loading page...");
    }

    fn destroy(self: Box<Self>, graphics: &mut graphics::graphics::Graphics) {}
}
