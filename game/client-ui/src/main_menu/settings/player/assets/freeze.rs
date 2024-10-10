use std::{collections::BTreeMap, time::Duration};

use game_interface::types::{
    character_info::NetworkSkinInfo,
    render::character::TeeEye,
    resource_key::{NetworkResourceKey, ResourceKey},
};
use ui_base::types::{UiRenderPipe, UiState};

use crate::{main_menu::user_data::UserData, utils::render_tee_for_ui_with_skin};

pub fn freeze_list(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    profile_index: usize,
) {
    let entries = pipe.user_data.freeze_container.entries_index();
    let entries_sorted = entries.into_iter().collect::<BTreeMap<_, _>>();
    let player = &mut pipe.user_data.config.game.players[profile_index];
    let search_str = pipe
        .user_data
        .config
        .engine
        .ui
        .path
        .query
        .entry("freeze-search".to_string())
        .or_default();
    let mut next_name = None;
    super::super::super::list::list::render(
        ui,
        entries_sorted.iter().map(|(name, &ty)| (name.as_str(), ty)),
        100.0,
        |_, name| {
            let valid: Result<NetworkResourceKey<24>, _> = name.try_into();
            valid.map(|_| ()).map_err(|err| err.into())
        },
        |_, name| player.freeze == name,
        |ui, _, name, pos, asset_size| {
            let skin_info = NetworkSkinInfo::Original;
            let key: ResourceKey = name.try_into().unwrap_or_default();
            render_tee_for_ui_with_skin(
                pipe.user_data.canvas_handle,
                pipe.user_data
                    .freeze_container
                    .get_or_default(&key)
                    .skin
                    .clone(),
                pipe.user_data.render_tee,
                ui,
                ui_state,
                pipe.user_data.full_rect,
                Some(ui.clip_rect()),
                Some(&skin_info),
                pos,
                asset_size,
                TeeEye::Normal,
            );
        },
        |_, name| {
            next_name = Some(name.to_string());
        },
        search_str,
        |_| {},
    );
    if let Some(next_name) = next_name.take() {
        player.freeze = next_name;
        pipe.user_data
            .player_settings_sync
            .set_player_info_changed();
    }
    pipe.user_data.freeze_container.update(
        &pipe.cur_time,
        &Duration::from_secs(10),
        &Duration::from_secs(1),
        [].iter(),
    );
}
