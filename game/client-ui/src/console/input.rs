use config::traits::ConfigInterface;
use egui::Layout;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// console input
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
    has_text_selection: bool,
) {
    let mouse_is_down = ui.input(|i| i.any_touches() || i.pointer.any_down());
    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.label(">");
        ui.with_layout(
            Layout::left_to_right(egui::Align::Center).with_main_justify(true),
            |ui| {
                let label = egui::TextEdit::singleline(pipe.user_data.msg)
                    .id_source("console-input")
                    .show(ui);
                if label.response.lost_focus() && !pipe.user_data.msg.is_empty() {
                    let mut splits = pipe.user_data.msg.split(" ");
                    let (path, val) = (splits.next(), splits.next());
                    pipe.config
                        .set_from_str(
                            path.map(|s| s.to_string()).unwrap_or_default(),
                            val.map(|s| s.to_string()).unwrap_or_default(),
                        )
                        .unwrap_or_else(|err| {
                            pipe.user_data
                                .msgs
                                .push_str(&format!("Parsing error: {}\n", err.to_string()));
                        });
                    pipe.user_data.msg.clear();
                    // TODO:
                } else if !label.response.has_focus() {
                    if (!mouse_is_down && !has_text_selection) || ui_state.hint_had_input {
                        label.response.request_focus();
                    }
                }
            },
        );
    });
}
