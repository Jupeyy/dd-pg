use egui::{Button, Frame, Layout};
use ui_base::types::UiRenderPipe;

use crate::ingame_menu::{constants::INGAME_MENU_VOTE_QUERY, user_data::UserData};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    Frame::central_panel(ui.style()).show(ui, |ui| {
        ui.with_layout(
            Layout::top_down(egui::Align::Min)
                .with_main_align(egui::Align::Min)
                .with_main_justify(true)
                .with_cross_justify(true)
                .with_main_wrap(true),
            |ui| {
                let current_active = pipe
                    .user_data
                    .browser_menu
                    .config
                    .engine
                    .ui
                    .path
                    .query
                    .entry(INGAME_MENU_VOTE_QUERY.to_string())
                    .or_insert_with(|| "Map".to_string());
                ui.horizontal_top(|ui| {
                    let mut btn = |name: &str| {
                        if ui
                            .add(Button::new(name).selected(current_active == name))
                            .clicked()
                        {
                            *current_active = name.to_string();
                        }
                    };
                    btn("Map");
                    btn("Player");
                    btn("Misc");
                });

                match current_active.as_str() {
                    "Map" => super::map::render(ui, pipe),
                    "Player" => super::players::render(ui, pipe),
                    // Misc
                    _ => {}
                }
            },
        );
    });
}
