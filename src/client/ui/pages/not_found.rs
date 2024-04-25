use game_config::config::Config;
use ui_traits::traits::UIRenderCallbackFunc;

pub struct Error404Page {}

impl Error404Page {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<Config> for Error404Page {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<Config>,
        ui_state: &mut ui_base::types::UIState,
    ) {
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut ui_base::types::UIPipe<Config>,
        _ui_state: &mut ui_base::types::UIState,
    ) {
        ui.label("Error 404: not found");
        if ui.button("return").clicked() {
            pipe.user_data.engine.ui.path.try_route("", "");
        }
    }
}
