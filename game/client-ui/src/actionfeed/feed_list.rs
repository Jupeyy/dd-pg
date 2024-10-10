use std::time::Duration;

use client_types::actionfeed::Action;
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
    ui.with_layout(Layout::top_down(egui::Align::Max), |ui| {
        // active input comes first (most bottom)
        for msg in pipe.user_data.entries.iter() {
            let time_diff = pipe.cur_time.saturating_sub(msg.add_time);
            if time_diff < Duration::from_secs(10) {
                let chat_fade = if time_diff >= Duration::from_secs(9) {
                    // re-render while opacity changes
                    ui.ctx().request_repaint();
                    1.0 - (time_diff.as_secs_f32() - 9.0)
                } else {
                    // re-render if opacity will change
                    ui.ctx()
                        .request_repaint_after(Duration::from_secs(9) - time_diff);
                    1.0
                };
                ui.set_opacity(chat_fade);
                match &msg.action {
                    Action::Kill(kill) => {
                        super::kill_entry::render(ui, pipe, ui_state, kill, full_rect);
                    }
                    Action::RaceFinish {
                        player,
                        finish_time,
                    } => {
                        super::race_finish_entry::render(
                            ui,
                            pipe,
                            ui_state,
                            std::slice::from_ref(player),
                            &player.name,
                            finish_time,
                            full_rect,
                        );
                    }
                    Action::RaceTeamFinish {
                        players,
                        team_name,
                        finish_time,
                    } => {
                        super::race_finish_entry::render(
                            ui,
                            pipe,
                            ui_state,
                            players,
                            team_name,
                            finish_time,
                            full_rect,
                        );
                    }
                    Action::Custom(_) => todo!(),
                };
            }
        }
    });
}
