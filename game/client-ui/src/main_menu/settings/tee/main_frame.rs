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

use crate::{main_menu::user_data::UserData, utils::render_tee_for_ui};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let player_index = 0;

        ui.horizontal(|ui| {
            ui.label("Player");
            ui.label("TODO: Dummy - use the multi player concept");
        });
        ui.add_space(5.0);

        let config = &mut pipe.user_data.config.game;
        let player = &mut config.players[player_index];

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
            clearable_edit_field(ui, &mut player.skin.name, Some(skin_size + 20.0));
        });
        let resource_key: Result<NetworkResourceKey<24>, _> = player.skin.name.as_str().try_into();
        ui.colored_label(
            Color32::RED,
            if let Err(err) = &resource_key {
                format!(
                    "Error: A valid skin name must only contain [0-9,a-z,A-Z], \
                        \"_\" or \" \" (space) characters in their name and \
                        must not exceed the 24 character limit. Actual error: {err}"
                )
            } else {
                "".to_string()
            },
        );
        let entries = pipe.user_data.skin_container.entries_index();
        let entries_sorted = entries.into_iter().collect::<BTreeMap<_, _>>();
        super::list::list::render(ui, pipe, ui_state, entries_sorted, player_index);
    });
    pipe.user_data.skin_container.update(
        &pipe.cur_time,
        &Duration::from_secs(10),
        &Duration::from_secs(1),
        [].iter(),
    );
}
