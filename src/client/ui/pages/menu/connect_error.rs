use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;
use ui_wasm_manager::UIWinitWrapper;

use ui_base::types::{UIPipe, UIState};

pub struct ConnectErrorMenu {}

impl ConnectErrorMenu {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend> for ConnectErrorMenu {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UIWinitWrapper>,
        _ui_state: &mut UIState<UIWinitWrapper>,
        _graphics: &mut Graphics,
    ) {
        ui.vertical(|ui| {
            ui.label(&format!(
                "connecting to addr failed {}: {}",
                pipe.config.ui.last_server_addr,
                pipe.ui_feedback.network_err()
            ));
            if ui.button("return").clicked() {
                pipe.ui_feedback.network_disconnect();
                pipe.ui_feedback.call_path(pipe.config, "", "");
            }
        });
    }
}
