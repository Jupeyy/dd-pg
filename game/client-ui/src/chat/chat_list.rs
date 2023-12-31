use client_types::chat::ServerMsg;
use egui::{Layout, Rect};
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// frame for the chat entries
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
    full_rect: &Rect,
) {
    ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
        // active input comes first (most bottom)
        super::input::render(ui, pipe, ui_state, graphics);
        for msg in pipe.user_data.entries.iter().rev() {
            match msg {
                ServerMsg::Chat(msg) => {
                    super::chat_entry::render(ui, pipe, ui_state, graphics, msg, full_rect);
                }
                ServerMsg::System(msg) => {
                    ui.label(&msg.msg);
                }
            };
        }
    });
}
