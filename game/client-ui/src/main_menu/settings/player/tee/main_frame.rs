use std::{collections::BTreeMap, time::Duration};

use egui::{Color32, Layout};
use game_interface::types::{
    render::character::TeeEye,
    resource_key::{NetworkResourceKey, ResourceKey},
};
use math::math::vector::vec2;
use ui_base::{
    components::clearable_edit_field::clearable_edit_field,
    types::{UiRenderPipe, UiState},
};

use crate::{
    main_menu::{settings::player::profile_selector::profile_selector, user_data::UserData},
    utils::render_tee_for_ui,
};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let config = &mut pipe.user_data.config.game;

        let profile_index = profile_selector(
            ui,
            "skin-profile-selection",
            config,
            &mut pipe.user_data.config.engine,
        );
        ui.add_space(5.0);
        let player = &mut config.players[profile_index as usize];

        ui.label("Preview:");
        let skin_size = 100.0;
        let rect = ui.available_rect_before_wrap();
        let pos = vec2::new(rect.min.x + skin_size / 2.0, rect.min.y + skin_size / 2.0);
        render_tee_for_ui(
            pipe.user_data.canvas_handle,
            pipe.user_data.skin_container,
            pipe.user_data.render_tee,
            ui,
            ui_state,
            pipe.user_data.full_rect,
            Some(ui.clip_rect()),
            &ResourceKey::from_str_lossy(&player.skin.name),
            Some(&(&player.skin).into()),
            pos,
            skin_size,
            TeeEye::Normal,
        );
        ui.add_space(skin_size);
        ui.horizontal(|ui| {
            clearable_edit_field(ui, &mut player.skin.name, Some(skin_size + 20.0), Some(24));
        });
        let resource_key: Result<NetworkResourceKey<24>, _> = player.skin.name.as_str().try_into();
        ui.colored_label(
            Color32::RED,
            if let Err(err) = &resource_key {
                format!(
                    "Error: A valid skin name must only contain [0-9,a-z,A-Z], \
                        \"_\" or \"-\" characters in their name and \
                        must not exceed the 24 character limit. Actual error: {err}"
                )
            } else {
                "".to_string()
            },
        );
        let entries = pipe.user_data.skin_container.entries_index();
        let entries_sorted = entries.into_iter().collect::<BTreeMap<_, _>>();
        let player = &mut pipe.user_data.config.game.players[profile_index as usize];
        let skin_search = pipe
            .user_data
            .config
            .engine
            .ui
            .path
            .query
            .entry("skin-search".to_string())
            .or_default();
        let mut next_name = None;
        super::super::super::list::list::render(
            ui,
            entries_sorted.iter().map(|(name, &ty)| (name.as_str(), ty)),
            100.0,
            |_, name| {
                let skin_valid: Result<NetworkResourceKey<24>, _> = name.try_into();
                skin_valid.map(|_| ()).map_err(|err| err.into())
            },
            |_, name| player.skin.name == name,
            |ui, _, name, pos, skin_size| {
                let skin_info = (&player.skin).into();
                render_tee_for_ui(
                    pipe.user_data.canvas_handle,
                    pipe.user_data.skin_container,
                    pipe.user_data.render_tee,
                    ui,
                    ui_state,
                    pipe.user_data.full_rect,
                    Some(ui.clip_rect()),
                    &name.try_into().unwrap_or_default(),
                    Some(&skin_info),
                    pos,
                    skin_size,
                    TeeEye::Normal,
                );
            },
            |_, name| {
                next_name = Some(name.to_string());
            },
            skin_search,
            |_| {},
        );
        if let Some(next_name) = next_name.take() {
            player.skin.name = next_name;
            pipe.user_data
                .player_settings_sync
                .set_player_info_changed();
        }
    });
    pipe.user_data.skin_container.update(
        &pipe.cur_time,
        &Duration::from_secs(10),
        &Duration::from_secs(1),
        [].iter(),
    );
}
