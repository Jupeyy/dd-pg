use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;

use ui_base::types::{UIPipe, UIState};

pub struct ConnectingMenu {}

impl ConnectingMenu {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<(), GraphicsBackend> for ConnectingMenu {
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
        pipe: &mut UIPipe<()>,
        _ui_state: &mut UIState,
        _graphics: &mut Graphics,
    ) {
        ui.vertical(|ui| {
            ui.label(&format!(
                "connecting addr to {}",
                pipe.config.ui.last_server_addr
            ));
            if ui.button("cancel").clicked() {
                pipe.ui_feedback.network_disconnect();
                pipe.ui_feedback.call_path(pipe.config, "", "");
            }
        });
    }
}
