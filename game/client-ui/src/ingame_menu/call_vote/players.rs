use egui::{Frame, Sense};
use egui_extras::{Column, TableBuilder};
use ui_base::types::UiRenderPipe;

use crate::{events::UiEvent, ingame_menu::user_data::UserData};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    pipe.user_data.server_players.request_player_infos();
    let server_players: Vec<_> = pipe
        .user_data
        .server_players
        .collect()
        .into_iter()
        .collect();

    let index_entry = pipe
        .user_data
        .browser_menu
        .config
        .engine
        .ui
        .path
        .query
        .entry("vote-player-index".to_string())
        .or_default();
    let index: usize = index_entry.parse().unwrap_or_default();

    Frame::window(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            TableBuilder::new(ui)
                .auto_shrink([false, false])
                .columns(Column::remainder(), 1)
                .sense(Sense::click())
                .header(30.0, |mut row| {
                    row.col(|ui| {
                        ui.label("Name");
                    });
                })
                .body(|body| {
                    body.rows(25.0, server_players.len(), |mut row| {
                        row.set_selected(index == row.index());
                        let (_, char) = &server_players[row.index()];
                        row.col(|ui| {
                            ui.label(char.name.as_str());
                        });
                        if row.response().clicked() {
                            *index_entry = row.index().to_string();
                        }
                    })
                });

            ui.horizontal(|ui| {
                if ui.button("kick").clicked() {
                    if let Some((id, _)) = server_players.get(index) {
                        pipe.user_data
                            .browser_menu
                            .events
                            .push(UiEvent::VoteKickPlayer {
                                voted_player_id: *id,
                            });
                    }
                }
                if ui.button("move to spec").clicked() {
                    if let Some((id, _)) = server_players.get(index) {
                        pipe.user_data
                            .browser_menu
                            .events
                            .push(UiEvent::VoteSpecPlayer {
                                voted_player_id: *id,
                            });
                    }
                }
            });
        });
    });
}
