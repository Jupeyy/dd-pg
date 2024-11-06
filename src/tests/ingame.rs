use std::time::Duration;

use client_containers::utils::RenderGameContainers;
use client_render_base::{
    map::render_pipe::{Camera, GameTimeInfo},
    render::particle_manager::ParticleManager,
};
use client_render_game::components::{
    game_objects::{GameObjectsRender, GameObjectsRenderPipe},
    players::{PlayerRenderPipe, Players},
};
use game_interface::types::{
    character_info::{NetworkCharacterInfo, NetworkSkinInfo},
    id_gen::IdGenerator,
    id_types::CharacterId,
    render::character::{CharacterInfo, CharacterRenderInfo, TeeEye},
    weapons::WeaponType,
};
use graphics::graphics::graphics::Graphics;
use hashlink::LinkedHashMap;
use map::map::groups::{
    layers::{
        physics::{MapLayerPhysics, MapLayerTilePhysicsBase},
        tiles::{Tile, TileFlags},
    },
    MapGroupPhysics, MapGroupPhysicsAttr,
};
use math::math::{vector::vec2, Rng};
use pool::{
    datatypes::{PoolLinkedHashMap, PoolString},
    rc::PoolRc,
};
use shared_game::collision::collision::Collision;
use ui_base::ui::UiCreator;

use super::utils::render_helper;

pub fn test_ingame(
    graphics: &Graphics,
    creator: &UiCreator,
    containers: &mut RenderGameContainers,
    save_screenshot: impl Fn(&str),
    player_count: usize,
    runs: usize,
) {
    let mut players = Players::new(graphics, creator);
    let mut game_objects = GameObjectsRender::new(graphics);
    let mut particles = ParticleManager::new(graphics, &Duration::ZERO);

    let mut character_infos: LinkedHashMap<CharacterId, CharacterInfo> = Default::default();
    let id_gen = IdGenerator::default();

    for _ in 0..player_count {
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

    let mut rng = Rng::new(0);
    let mut render_infos: LinkedHashMap<CharacterId, CharacterRenderInfo> = Default::default();
    for (char_id, _) in character_infos.iter() {
        render_infos.insert(
            *char_id,
            CharacterRenderInfo {
                lerped_pos: vec2::new(
                    rng.random_float_in(-10.0..=10.0),
                    rng.random_float_in(-10.0..=10.0),
                ),
                lerped_vel: Default::default(),
                lerped_hook_pos: None,
                has_air_jump: false,
                cursor_pos: Default::default(),
                move_dir: 0,
                cur_weapon: WeaponType::Hammer,
                recoil_ticks_passed: None,
                left_eye: TeeEye::Angry,
                right_eye: TeeEye::Angry,
                buffs: PoolLinkedHashMap::new_without_pool(),
                debuffs: PoolLinkedHashMap::new_without_pool(),
                animation_ticks_passed: 0,
                game_ticks_passed: 0,
                emoticon: None,
            },
        );
    }

    let mut time_offset = Duration::ZERO;
    let mut render = |base_name: &str| {
        let render_internal = |_i: u64, time_offset: Duration| {
            let game_time_info = GameTimeInfo {
                ticks_per_second: 50.try_into().unwrap(),
                intra_tick_time: Default::default(),
            };
            let camera = Camera {
                pos: Default::default(),
                zoom: 1.0,
            };

            game_objects.render(&mut GameObjectsRenderPipe {
                particle_manager: &mut particles,
                cur_time: &time_offset,
                game_time_info: &game_time_info,
                character_infos: &character_infos,
                projectiles: &LinkedHashMap::default(),
                flags: &LinkedHashMap::default(),
                lasers: &LinkedHashMap::default(),
                pickups: &LinkedHashMap::default(),
                ctf_container: &mut containers.ctf_container,
                game_container: &mut containers.game_container,
                ninja_container: &mut containers.ninja_container,
                weapon_container: &mut containers.weapon_container,
                local_character_id: None,
                camera: &camera,
            });

            players.render(&mut PlayerRenderPipe {
                cur_time: &time_offset,
                game_time_info: &game_time_info,
                render_infos: &render_infos,
                character_infos: &character_infos,
                skins: &mut containers.skin_container,
                ninjas: &mut containers.ninja_container,
                freezes: &mut containers.freeze_container,
                hooks: &mut containers.hook_container,
                weapons: &mut containers.weapon_container,
                emoticons: &mut containers.emoticons_container,
                particle_manager: &mut particles,
                collision: &Collision::new(
                    &MapGroupPhysics {
                        attr: MapGroupPhysicsAttr {
                            width: 1u16.try_into().unwrap(),
                            height: 1u16.try_into().unwrap(),
                        },
                        layers: vec![MapLayerPhysics::Game(MapLayerTilePhysicsBase {
                            tiles: vec![Tile {
                                flags: TileFlags::empty(),
                                index: 0,
                            }],
                        })],
                    },
                    false,
                )
                .unwrap(),
                camera: &camera,
                spartial_sound: false,
                sound_playback_speed: 1.0,
                ingame_sound_volume: 0.0,
                own_character: None,
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

    for _ in 0..runs {
        render("ingame");
    }
}
