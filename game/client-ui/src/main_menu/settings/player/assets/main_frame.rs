use egui::Layout;
use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::{settings::player::profile_selector::profile_selector, user_data::UserData};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let config = &mut pipe.user_data.config.game;

        let profile_index = profile_selector(
            ui,
            "assets-profile-selection",
            config,
            &mut pipe.user_data.config.engine,
        );
        ui.add_space(5.0);

        let assets_tab = pipe
            .user_data
            .config
            .engine
            .ui
            .path
            .query
            .entry("assets-tab".to_string())
            .or_default();

        ui.with_layout(
            Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                let mut add_btn = |name: &str| {
                    if ui.button(name).clicked() {
                        *assets_tab = name.to_string();
                    }
                };
                add_btn("weapons");
                add_btn("hook");
                add_btn("entities");
                add_btn("freeze");
                add_btn("emoticons");
                add_btn("particles");
                add_btn("ninja");
                add_btn("game");
                add_btn("hud");
                add_btn("ctf");
            },
        );

        match assets_tab.as_str() {
            "weapons" => {
                super::weapons::weapon_list(ui, pipe, ui_state, profile_index as usize);
            }
            "hook" => {
                super::hook::hook_list(ui, pipe, ui_state, profile_index as usize);
            }
            "entities" => {
                super::entities::entities_list(ui, pipe, ui_state, profile_index as usize);
            }
            "freeze" => {
                super::freeze::freeze_list(ui, pipe, ui_state, profile_index as usize);
            }
            "ninja" => {
                super::ninja::ninja_list(ui, pipe, ui_state, profile_index as usize);
            }
            "emoticons" => {
                super::emoticons::emoticons_list(ui, pipe, ui_state, profile_index as usize);
            }
            "particles" => {
                super::particles::particles_list(ui, pipe, ui_state, profile_index as usize);
            }
            "game" => {
                super::game::game_list(ui, pipe, ui_state, profile_index as usize);
            }
            "hud" => {
                super::hud::hud_list(ui, pipe, ui_state, profile_index as usize);
            }
            // ctf
            _ => {
                super::ctf::ctf_list(ui, pipe, ui_state, profile_index as usize);
            }
        }
    });
}
