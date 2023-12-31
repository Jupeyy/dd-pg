use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// chat input
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
) {
    if *pipe.user_data.is_input_active {
        ui.horizontal(|ui| {
            ui.label("All:");
            let label = ui.text_edit_singleline(pipe.user_data.msg);
            if label.lost_focus() {
                *pipe.user_data.is_input_active = false;
                pipe.user_data
                    .chat
                    .on_message(std::mem::take(pipe.user_data.msg));
            } else {
                label.request_focus();
            }
        });
    }
}
