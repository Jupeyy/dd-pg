use ui_base::types::{UIPipe, UIState};

use super::user_data::{ChatEvent, UserData};

/// chat input
pub fn render(ui: &mut egui::Ui, pipe: &mut UIPipe<UserData>, _ui_state: &mut UIState) {
    if pipe.user_data.is_input_active {
        ui.horizontal(|ui| {
            ui.label("All:");
            let label = ui.text_edit_singleline(pipe.user_data.msg);
            if label.lost_focus() {
                pipe.user_data.chat_events.push(ChatEvent::ChatClosed);
                pipe.user_data
                    .chat_events
                    .push(ChatEvent::MsgSend(pipe.user_data.msg.clone()));
            } else {
                pipe.user_data
                    .chat_events
                    .push(ChatEvent::CurMsg(pipe.user_data.msg.clone()));

                label.request_focus();
            }
        });
    }
}
