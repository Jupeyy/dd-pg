use egui::Frame;
use egui_extras::{Column, TableBuilder};
use ui_base::types::{UiRenderPipe, UiState};

use crate::ingame_menu::user_data::UserData;

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    _ui_state: &mut UiState,
    main_frame_only: bool,
) {
    pipe.user_data.server_players.request_player_infos();
    let server_players: Vec<_> = pipe
        .user_data
        .server_players
        .collect()
        .into_iter()
        .collect();
    Frame::window(ui.style()).show(ui, |ui| {
        TableBuilder::new(ui)
            .auto_shrink([false, false])
            .columns(Column::remainder(), 2)
            .header(30.0, |mut row| {
                row.col(|ui| {
                    ui.label("Name");
                });
                row.col(|ui| {
                    ui.label("Flag");
                });
            })
            .body(|body| {
                body.rows(25.0, server_players.len(), |mut row| {
                    let (id, char) = &server_players[row.index()];
                    row.col(|ui| {
                        ui.label(char.name.as_str());
                    });
                    row.col(|ui| {
                        ui.label("TODO:");
                    });
                })
            });
    });
}
