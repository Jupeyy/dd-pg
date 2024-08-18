use std::time::Duration;

use client_types::chat::ServerMsg;
use egui::{Layout, Rect};

use ui_base::types::{UiRenderPipe, UiState};

use super::user_data::UserData;

/// frame for the chat entries
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    full_rect: &Rect,
) {
    ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
        // active input comes first (most bottom)
        super::input::render(ui, pipe, ui_state);
        for msg in pipe.user_data.entries.iter().rev() {
            let time_diff = pipe.cur_time.saturating_sub(msg.add_time);
            if time_diff < Duration::from_secs(10) || pipe.user_data.show_chat_history {
                match &msg.msg {
                    ServerMsg::Chat(msg) => {
                        super::chat_entry::render(ui, pipe, ui_state, msg, full_rect);
                    }
                    ServerMsg::System(msg) => {
                        super::system_entry::render(ui, pipe, ui_state, msg);
                    }
                };
            }
        }
    });
}
