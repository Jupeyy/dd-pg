use game_config::config::Config;
use ui_traits::traits::UIRenderCallbackFunc;

pub struct LoadingPage {}

impl LoadingPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<Config> for LoadingPage {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        _ui: &mut egui::Ui,
        _pipe: &mut ui_base::types::UIPipe<Config>,
        _ui_state: &mut ui_base::types::UIState,
    ) {
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _pipe: &mut ui_base::types::UIPipe<Config>,
        _ui_state: &mut ui_base::types::UIState,
    ) {
        ui.label("Loading page...");
    }
}
