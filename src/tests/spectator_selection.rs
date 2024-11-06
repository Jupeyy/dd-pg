use std::time::Duration;

use client_containers::utils::RenderGameContainers;
use client_render::spectator_selection::page::{
    SpectatorSelectionRender, SpectatorSelectionRenderPipe,
};
use client_render_base::render::tee::RenderTee;
use game_interface::types::character_info::{NetworkCharacterInfo, NetworkSkinInfo};
use game_interface::types::id_gen::IdGenerator;
use game_interface::types::id_types::CharacterId;
use game_interface::types::render::character::{CharacterInfo, TeeEye};
use graphics::graphics::graphics::Graphics;
use hashlink::LinkedHashMap;
use pool::datatypes::PoolString;
use pool::rc::PoolRc;
use ui_base::ui::UiCreator;

use super::utils::render_helper;

pub fn test_spectator_selection(
    graphics: &Graphics,
    creator: &UiCreator,
    containers: &mut RenderGameContainers,
    render_tee: &RenderTee,
    save_screenshot: impl Fn(&str),
) {
    let mut spectator_selection = SpectatorSelectionRender::new(graphics, creator);

    let mut time_offset = Duration::ZERO;
    let mut render =
        |base_name: &str, character_infos: &LinkedHashMap<CharacterId, CharacterInfo>| {
            let render_internal = |_i: u64, time_offset: Duration| {
                spectator_selection.render(&mut SpectatorSelectionRenderPipe {
                    cur_time: &time_offset,
                    input: &mut None,
                    skin_container: &mut containers.skin_container,
                    skin_renderer: render_tee,
                    character_infos,
                });
            };
            render_helper(
                graphics,
                render_internal,
                &mut time_offset,
                base_name,
                &save_screenshot,
            );
        };

    render("spectator_selection_empty", &Default::default());

    let mut character_infos: LinkedHashMap<CharacterId, CharacterInfo> = Default::default();
    let id_gen = IdGenerator::default();
    for _ in 0..20 {
        character_infos.insert(
            id_gen.next_id(),
            CharacterInfo {
                info: PoolRc::from_item_without_pool(NetworkCharacterInfo::explicit_default()),
                skin_info: NetworkSkinInfo::Original,
                laser_info: Default::default(),
                stage_id: Some(id_gen.next_id()),
                side: None,
                player_info: None,
                browser_score: PoolString::new_without_pool(),
                browser_eye: TeeEye::Happy,
            },
        );
    }
    render("spectator_selection_20", &character_infos);
}
