use egui::{Frame, Sense};
use egui_extras::{Column, TableBuilder};
use ui_base::types::{UiRenderPipe, UiState};

use crate::{events::UiEvent, ingame_menu::user_data::UserData};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    _ui_state: &mut UiState,
    main_frame_only: bool,
) {
    pipe.user_data.votes.request_map_votes();
    let map_infos: Vec<_> = pipe
        .user_data
        .votes
        .collect_map_votes()
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
        .entry("vote-map-index".to_string())
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
                    body.rows(25.0, map_infos.len(), |mut row| {
                        row.set_selected(index == row.index());
                        let (_, map) = &map_infos[row.index()];
                        row.col(|ui| {
                            ui.label(map.name.as_str());
                        });
                        if row.response().clicked() {
                            *index_entry = row.index().to_string();
                        }
                    })
                });

            ui.horizontal(|ui| {
                if ui.button("change").clicked() {
                    if let Some((_, map)) = map_infos.get(index) {
                        pipe.user_data.browser_menu.events.push(UiEvent::VoteMap {
                            voted_map: map.clone(),
                        });
                    }
                }
            });
        });
    });
}
