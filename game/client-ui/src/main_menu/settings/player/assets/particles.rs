use std::{collections::BTreeMap, time::Duration};

use game_interface::types::resource_key::{NetworkResourceKey, ResourceKey};
use graphics::handles::texture::texture::TextureContainer;
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use crate::{main_menu::user_data::UserData, utils::render_texture_for_ui};

pub fn particles_list(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    profile_index: usize,
) {
    let entries = pipe.user_data.particles_container.entries_index();
    let entries_sorted = entries.into_iter().collect::<BTreeMap<_, _>>();
    let player = &mut pipe.user_data.config.game.players[profile_index];
    let search_str = pipe
        .user_data
        .config
        .engine
        .ui
        .path
        .query
        .entry("particles-search".to_string())
        .or_default();
    let mut next_name = None;
    super::super::super::list::list::render(
        ui,
        entries_sorted.iter().map(|(name, &ty)| (name.as_str(), ty)),
        150.0,
        |_, name| {
            let valid: Result<NetworkResourceKey<24>, _> = name.try_into();
            valid.map(|_| ()).map_err(|err| err.into())
        },
        |_, name| player.particles == name,
        |ui, _, name, pos, asset_size| {
            let item_size = asset_size / 4.0;
            let pos = pos
                + vec2::new(
                    item_size / 2.0 - (asset_size / 2.0),
                    item_size / 2.0 - (asset_size / 2.0),
                );
            let key: ResourceKey = name.try_into().unwrap_or_default();
            let particles = pipe.user_data.particles_container.get_or_default(&key);

            let mut render_texture = |texture: &TextureContainer, index: usize| {
                let x = (index % 4) as f32;
                let y = (index / 4) as f32;
                render_texture_for_ui(
                    pipe.user_data.stream_handle,
                    pipe.user_data.canvas_handle,
                    texture,
                    ui,
                    ui_state,
                    pipe.user_data.full_rect,
                    Some(ui.clip_rect()),
                    pos + vec2::new(x * item_size, y * item_size),
                    vec2::new(item_size, item_size),
                );
            };
            let mut index = 0;

            render_texture(&particles.airjump, index);
            index += 1;
            render_texture(&particles.slice, index);
            index += 1;
            render_texture(&particles.ball, index);
            index += 1;
            for tex in &particles.splats {
                render_texture(tex, index);
                index += 1;
            }
            render_texture(&particles.smoke, index);
            index += 1;
            render_texture(&particles.shell, index);
            index += 1;
            for tex in &particles.explosions {
                render_texture(tex, index);
                index += 1;
            }
            for tex in &particles.hits {
                render_texture(tex, index);
                index += 1;
            }
            for tex in &particles.stars {
                render_texture(tex, index);
                index += 1;
            }
        },
        |_, name| {
            next_name = Some(name.to_string());
        },
        search_str,
        |_| {},
    );
    if let Some(next_name) = next_name.take() {
        player.particles = next_name;
        pipe.user_data
            .player_settings_sync
            .set_player_info_changed();
    }

    pipe.user_data.particles_container.update(
        &pipe.cur_time,
        &Duration::from_secs(10),
        &Duration::from_secs(1),
        [].iter(),
    );
}
