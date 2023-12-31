use graphics::graphics::Graphics;
use ui_traits::traits::UIRenderCallbackFunc;

pub struct LoadingPage {}

impl LoadingPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<()> for LoadingPage {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        _ui: &mut egui::Ui,
        _pipe: &mut ui_base::types::UIPipe<()>,
        _ui_state: &mut ui_base::types::UIState,
        _graphics: &mut graphics::graphics::Graphics,
    ) {
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _pipe: &mut ui_base::types::UIPipe<()>,
        _ui_state: &mut ui_base::types::UIState,
        _graphics: &mut Graphics,
    ) {
        ui.label("Loading page...");
    }
}
