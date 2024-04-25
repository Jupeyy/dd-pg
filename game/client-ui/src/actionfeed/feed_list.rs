use client_types::actionfeed::ActionFeed;
use egui::{Layout, Rect};

use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// frame for the chat entries
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    full_rect: &Rect,
) {
    ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
        // active input comes first (most bottom)
        for msg in pipe.user_data.entries.iter().rev() {
            match msg {
                ActionFeed::Kill(kill) => {
                    super::kill_entry::render(ui, pipe, ui_state, kill, full_rect);
                }
                client_types::actionfeed::ActionFeed::RaceFinish {
                    players,
                    finish_time,
                } => todo!(),
                client_types::actionfeed::ActionFeed::Custom(_) => todo!(),
            };
        }
    });
}
