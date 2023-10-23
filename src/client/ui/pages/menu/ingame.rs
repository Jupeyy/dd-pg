use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;

use ui_base::types::{UIPipe, UIState};

pub struct IngameMenu {}

impl IngameMenu {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<(), GraphicsBackend> for IngameMenu {
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
        ui_state: &mut UIState,
        _graphics: &mut Graphics,
    ) {
        let dummies_connected = pipe.ui_feedback.local_player_count();
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui_state.is_ui_open = false;
        }
        ui.horizontal(|ui| {
            if ui.button("disconnect").clicked() {
                pipe.ui_feedback.network_disconnect();
                pipe.ui_feedback.call_path(pipe.config, "", "");
            }
            if ui
                .button(&format!("connect dummy ({})", dummies_connected.max(1) - 1))
                .clicked()
            {
                pipe.ui_feedback.network_connect_local_player();
            }
            if ui.button("disconnect dummy").clicked() {
                pipe.ui_feedback.network_disconnect_local_player();
            }
        });
    }
}
