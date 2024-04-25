use std::{sync::Arc, time::Duration};

use crate::{
    components::{
        cursor::{RenderCursor, RenderCursorPipe},
        effects::Effects,
        game_objects::{GameObjectsRender, GameObjectsRenderPipe},
        hud::{RenderHud, RenderHudPipe},
        particle_manager::{ParticleGroup, ParticleManager},
        players::{PlayerRenderPipe, Players},
    },
    map::render_map_base::{ClientMapRender, RenderMapLoading},
};
use base_io::io::IO;
use base_log::log::SystemLog;
use client_containers_new::{
    ctf::{CTFContainer, CTF_CONTAINER_PATH},
    emoticons::{EmoticonsContainer, EMOTICONS_CONTAINER_PATH},
    entities::{EntitiesContainer, ENTITIES_CONTAINER_PATH},
    freezes::{FreezeContainer, FREEZE_CONTAINER_PATH},
    game::{GameContainer, GAME_CONTAINER_PATH},
    hooks::{HookContainer, HOOK_CONTAINER_PATH},
    hud::{HudContainer, HUD_CONTAINER_PATH},
    ninja::{NinjaContainer, NINJA_CONTAINER_PATH},
    particles::{ParticlesContainer, PARTICLES_CONTAINER_PATH},
    skins::{SkinContainer, SKIN_CONTAINER_PATH},
    weapons::{WeaponContainer, WEAPON_CONTAINER_PATH},
};
use client_render::{
    actionfeed::render::{ActionfeedRender, ActionfeedRenderPipe},
    chat::render::{ChatRender, ChatRenderOptions, ChatRenderPipe},
    scoreboard::render::{ScoreboardRender, ScoreboardRenderPipe},
};
use client_render_base::map::render_pipe::{Camera, GameStateRenderInfo, RenderPipeline};
use client_types::{
    actionfeed::{ActionFeed, ActionFeedKill, ActionFeedKillWeapon, ActionFeedPlayer},
    chat::{ChatMsg, ChatMsgPlayerChannel, MsgSystem, ServerMsg},
};
use client_ui::chat::user_data::ChatEvent;
use config::config::{ConfigDebug, ConfigEngine};
use game_config::config::ConfigMap;
use game_interface::{
    events::{
        GameBuffEvent, GameBuffNinjaEvent, GameBuffNinjaEventSound, GameCharacterEvent,
        GameCharacterEventEffect, GameCharacterEventSound, GameDebuffEvent, GameDebuffFrozenEvent,
        GameDebuffFrozenEventSound, GameEvents, GameFlagEvent, GameFlagEventSound,
        GameGrenadeEvent, GameGrenadeEventEffect, GameGrenadeEventSound, GameLaserEvent,
        GameLaserEventSound, GamePickupArmorEvent, GamePickupArmorEventSound, GamePickupEvent,
        GamePickupHeartEvent, GamePickupHeartEventSound, GameShotgunEvent, GameShotgunEventSound,
        GameWorldActionFeed, GameWorldEntityEvent, GameWorldEvent, GameWorldGlobalEvent,
        GameWorldPositionedEvent, GameWorldSystemMessage,
    },
    types::{
        flag::FlagType,
        game::GameEntityId,
        render::{
            character::{
                CharacterBuff, CharacterInfo, CharacterRenderInfo, LocalCharacterRenderInfo,
            },
            flag::FlagRenderInfo,
            laser::LaserRenderInfo,
            pickup::PickupRenderInfo,
            projectiles::ProjectileRenderInfo,
            scoreboard::ScoreboardGameType,
        },
    },
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle},
};
use graphics_types::rendering::ColorRGBA;
use hashlink::LinkedHashMap;
use math::math::{vector::vec2, Rng};
use pool::{
    datatypes::{PoolLinkedHashMap, PoolVec, PoolVecDeque},
    pool::Pool,
};
use rayon::ThreadPool;
use serde::{Deserialize, Serialize};
use shared_base::network::types::chat::NetChatMsg;
use sound::{
    scene_object::SceneObject, sound::SoundManager, sound_listener::SoundListener,
    types::SoundPlayProps,
};

#[derive(Default, Serialize, Deserialize)]
pub struct RenderGameResult {
    pub player_events: LinkedHashMap<GameEntityId, Vec<ChatEvent>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderForPlayer {
    pub chat_info: Option<(String, Option<egui::RawInput>)>,
    pub local_player_info: LocalCharacterRenderInfo,
    pub scoreboard_active: bool,
    pub player_id: GameEntityId,
}

#[derive(Serialize, Deserialize)]
pub struct RenderGameForPlayer {
    pub render_for_player: Option<RenderForPlayer>,
    pub game_state_info: GameStateRenderInfo,
}

#[derive(Serialize, Deserialize)]
pub struct RenderGameInput {
    pub players: PoolVec<RenderGameForPlayer>,
    pub events: GameEvents,
    pub chat_msgs: PoolVecDeque<NetChatMsg>,

    pub character_render_infos: PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>,
    pub character_infos: PoolLinkedHashMap<GameEntityId, CharacterInfo>,
    pub projectiles: PoolVec<ProjectileRenderInfo>,
    pub flags: PoolVec<FlagRenderInfo>,
    pub lasers: PoolVec<LaserRenderInfo>,
    pub pickups: PoolVec<PickupRenderInfo>,
    pub scoreboard_info: ScoreboardGameType,
}

pub struct RenderGame {
    // containers
    skin_container: SkinContainer,
    weapon_container: WeaponContainer,
    hook_container: HookContainer,
    ctf_container: CTFContainer,
    ninja_container: NinjaContainer,
    freeze_container: FreezeContainer,
    entities_container: EntitiesContainer,
    hud_container: HudContainer,
    emoticons_container: EmoticonsContainer,
    particles_container: ParticlesContainer,
    game_container: GameContainer,

    // render components
    players: Players,
    render: GameObjectsRender,
    cursor_render: RenderCursor,
    pub chat: ChatRender,
    actionfeed: ActionfeedRender,
    scoreboard: ScoreboardRender,
    hud: RenderHud,
    particles: ParticleManager,

    // map
    map: ClientMapRender,

    canvas_handle: GraphicsCanvasHandle,
    backend_handle: GraphicsBackendHandle,

    // helpers
    helper: Pool<Vec<(i32, i32, u32, u32, RenderGameForPlayer)>>,

    world_sound_scene: SceneObject,
    world_sound_listener: SoundListener,
    rng: Rng,
}

impl RenderGame {
    pub fn new(
        sound: &SoundManager,
        graphics: &Graphics,
        io: &IO,
        thread_pool: &Arc<ThreadPool>,
        cur_time: &Duration,
        sys_log: &Arc<SystemLog>,
        map_file: Vec<u8>,
        config: &ConfigEngine,
    ) -> Self {
        let scene = sound.scene_handle.create();

        let default_skin = SkinContainer::load_default(io, SKIN_CONTAINER_PATH.as_ref());
        let default_weapon = WeaponContainer::load_default(io, WEAPON_CONTAINER_PATH.as_ref());
        let default_hook = HookContainer::load_default(io, HOOK_CONTAINER_PATH.as_ref());
        let default_ctf = CTFContainer::load_default(io, CTF_CONTAINER_PATH.as_ref());
        let default_ninja = NinjaContainer::load_default(io, NINJA_CONTAINER_PATH.as_ref());
        let default_freeze = FreezeContainer::load_default(io, FREEZE_CONTAINER_PATH.as_ref());
        let default_entities =
            EntitiesContainer::load_default(io, ENTITIES_CONTAINER_PATH.as_ref());
        let default_hud = HudContainer::load_default(io, HUD_CONTAINER_PATH.as_ref());
        let default_emoticons =
            EmoticonsContainer::load_default(io, EMOTICONS_CONTAINER_PATH.as_ref());
        let default_particles =
            ParticlesContainer::load_default(io, PARTICLES_CONTAINER_PATH.as_ref());
        let default_games = GameContainer::load_default(io, GAME_CONTAINER_PATH.as_ref());

        let map = ClientMapRender::new(RenderMapLoading::new(
            thread_pool.clone(),
            map_file,
            io.clone(),
            sound,
            graphics,
            config,
        ));

        let resource_http_download_url = None;
        let resource_server_download_url = None;

        let skin_container = SkinContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_skin,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "skin-container",
            graphics,
            sound,
            &scene,
            SKIN_CONTAINER_PATH.as_ref(),
        );
        let weapon_container = WeaponContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_weapon,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "weapon-container",
            graphics,
            sound,
            &scene,
            WEAPON_CONTAINER_PATH.as_ref(),
        );
        let hook_container = HookContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_hook,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "hook-container",
            graphics,
            sound,
            &scene,
            HOOK_CONTAINER_PATH.as_ref(),
        );
        let ctf_container = CTFContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_ctf,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "ctf-container",
            graphics,
            sound,
            &scene,
            CTF_CONTAINER_PATH.as_ref(),
        );
        let ninja_container = NinjaContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_ninja,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "ninja-container",
            graphics,
            sound,
            &scene,
            NINJA_CONTAINER_PATH.as_ref(),
        );
        let freeze_container = FreezeContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_freeze,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "freeze-container",
            graphics,
            sound,
            &scene,
            FREEZE_CONTAINER_PATH.as_ref(),
        );
        let entities_container = EntitiesContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_entities,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "entities-container",
            graphics,
            sound,
            &scene,
            ENTITIES_CONTAINER_PATH.as_ref(),
        );
        let hud_container = HudContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_hud,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "hud-container",
            graphics,
            sound,
            &scene,
            HUD_CONTAINER_PATH.as_ref(),
        );
        let emoticons_container = EmoticonsContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_emoticons,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "emoticons-container",
            graphics,
            sound,
            &scene,
            EMOTICONS_CONTAINER_PATH.as_ref(),
        );
        let particles_container = ParticlesContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_particles,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "particles-container",
            graphics,
            sound,
            &scene,
            PARTICLES_CONTAINER_PATH.as_ref(),
        );
        let game_container = GameContainer::new(
            io.clone(),
            thread_pool.clone(),
            default_games,
            sys_log,
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "games-container",
            graphics,
            sound,
            &scene,
            GAME_CONTAINER_PATH.as_ref(),
        );

        let players = Players::new(graphics);
        let render = GameObjectsRender::new(cur_time, graphics);
        let cursor_render = RenderCursor::new(graphics);
        let hud = RenderHud::new(graphics);
        let particles = ParticleManager::new(graphics, cur_time);

        let chat = ChatRender::new(graphics);
        let actionfeed = ActionfeedRender::new(graphics);
        let scoreboard = ScoreboardRender::new(graphics);

        let listener = scene.sound_listener_handle.create(vec2::default());

        Self {
            // entities
            skin_container,
            weapon_container,
            hook_container,
            ctf_container,
            ninja_container,
            freeze_container,
            entities_container,
            hud_container,
            emoticons_container,
            particles_container,
            game_container,

            // components
            players,
            render,
            cursor_render,
            chat,
            actionfeed,
            scoreboard,
            hud,
            particles,

            map,

            canvas_handle: graphics.canvas_handle.clone(),
            backend_handle: graphics.backend_handle.clone(),

            helper: Pool::with_capacity(1),

            world_sound_scene: scene,
            world_sound_listener: listener,
            rng: Rng::new(0),
        }
    }

    fn render_ingame(
        &mut self,

        config_map: &ConfigMap,
        cur_time: &Duration,

        render_info: &RenderGameInput,
        player_info: &RenderGameForPlayer,
    ) {
        let map = self.map.try_get().unwrap();

        let mut cam = Camera {
            pos: Default::default(),
            zoom: 1.0,
            animation_ticks_passed: Default::default(),
        };
        if let Some(local_render_info) = &player_info.render_for_player {
            if let Some(character) = render_info
                .character_render_infos
                .get(&local_render_info.player_id)
            {
                cam.pos = character.lerped_pos;
                cam.animation_ticks_passed = character.animation_ticks_passed;
            }
        }

        let render_map = map;

        // map + ingame objects
        let mut render_pipe = RenderPipeline::new(
            &render_map.data.buffered_map.map_visual,
            &render_map.data.buffered_map,
            config_map,
            cur_time,
            &player_info.game_state_info,
            &cam,
            &mut self.entities_container,
        );
        render_map
            .render
            .render_background(&render_map.data.buffered_map.map_visual, &mut render_pipe);
        self.particles.render_group(
            ParticleGroup::ProjectileTrail,
            &mut self.particles_container,
            &cam,
        );
        self.render.render(&mut GameObjectsRenderPipe {
            particle_manager: &mut self.particles,
            cur_time,
            game_info: &player_info.game_state_info,

            projectiles: &render_info.projectiles,
            flags: &render_info.flags,
            pickups: &render_info.pickups,
            lasers: &render_info.lasers,

            ctf_container: &mut self.ctf_container,
            game_container: &mut self.game_container,
            ninja_container: &mut self.ninja_container,
            weapon_container: &mut self.weapon_container,

            camera: &cam,
        });
        self.players.render(&mut PlayerRenderPipe {
            cur_time,
            game_info: &player_info.game_state_info,
            render_infos: &render_info.character_render_infos,
            character_infos: &render_info.character_infos,

            particle_manager: &mut self.particles,

            skins: &mut self.skin_container,
            ninjas: &mut self.ninja_container,
            hooks: &mut self.hook_container,
            weapons: &mut self.weapon_container,
            emoticons: &mut self.emoticons_container,

            collision: &render_map.data.collision,
            camera: &cam,
        });
        let mut render_pipe = RenderPipeline::new(
            &render_map.data.buffered_map.map_visual,
            &render_map.data.buffered_map,
            config_map,
            cur_time,
            &player_info.game_state_info,
            &cam,
            &mut self.entities_container,
        );
        render_map.render.render_physics_layers(
            &render_map.data.buffered_map.map_visual,
            &mut render_pipe.base,
            &render_map.data.buffered_map.render.physics_render_layers,
        );
        render_map
            .render
            .render_foreground(&render_map.data.buffered_map.map_visual, &mut render_pipe);

        self.particles.render_groups(
            ParticleGroup::Explosions,
            &mut self.particles_container,
            &cam,
        );
        // cursor
        if let Some(local_render_info) = &player_info.render_for_player {
            if let Some(player) = render_info
                .character_render_infos
                .get(&local_render_info.player_id)
            {
                self.cursor_render.render(&mut RenderCursorPipe {
                    mouse_cursor: player.cursor_pos,
                    weapon_container: &mut self.weapon_container,
                    cur_weapon: player.cur_weapon,
                    is_ninja: player.buffs.contains_key(&CharacterBuff::Ninja),
                    ninja_container: &mut self.ninja_container,
                });
            }
        }
    }

    /// render hud + uis: chat, scoreboard etc.
    #[must_use]
    fn render_uis(
        &mut self,

        cur_time: &Duration,

        render_info: &RenderGameInput,
        player_info: &mut RenderGameForPlayer,
    ) -> Vec<ChatEvent> {
        let mut res: Vec<ChatEvent> = Default::default();
        // chat
        if let Some(chat_render_info) = &mut player_info.render_for_player {
            let mut dummy_str: String = Default::default();
            let mut dummy_str_ref = &mut dummy_str;
            let mut dummy_state = &mut None;

            let chat_active = if let Some((chat_msg, chat_state)) = &mut chat_render_info.chat_info
            {
                dummy_str_ref = chat_msg;
                dummy_state = chat_state;
                true
            } else {
                false
            };

            res = self.chat.render(&mut ChatRenderPipe {
                cur_time,
                msg: dummy_str_ref,
                options: ChatRenderOptions {
                    is_chat_input_active: chat_active,
                    is_chat_show_all: false, // TODO:
                },
                ui_pipe: dummy_state,
                player_id: &chat_render_info.player_id,
                skin_container: &mut self.skin_container,
                tee_render: &mut self.players.tee_renderer,
            });
        }

        // action feed
        self.actionfeed
            .render(&mut ActionfeedRenderPipe { cur_time });

        // hud + scoreboard
        if let Some(local_render_info) = &player_info.render_for_player {
            self.hud.render(&mut RenderHudPipe {
                hud_container: &mut self.hud_container,
                weapon_container: &mut self.weapon_container,
                local_player_render_info: &local_render_info.local_player_info,
                cur_weapon: render_info
                    .character_render_infos
                    .get(&local_render_info.player_id)
                    .map(|player| player.cur_weapon)
                    .unwrap_or_default(),
            });
            if local_render_info.scoreboard_active {
                // scoreboard after hud
                self.scoreboard.render(&mut ScoreboardRenderPipe {
                    cur_time,
                    entries: &render_info.scoreboard_info,
                    character_infos: &render_info.character_infos,
                    skin_container: &mut self.skin_container,
                    tee_render: &mut self.players.tee_renderer,
                });
            }
        }
        res
    }
}

pub trait RenderGameInterface {
    fn render(
        &mut self,
        config_map: &ConfigMap,
        cur_time: &Duration,
        input: RenderGameInput,
    ) -> RenderGameResult;
    fn continue_map_loading(&mut self, config: &ConfigDebug) -> bool;
}

impl RenderGame {
    fn convert_system_ev(ev: GameWorldSystemMessage) -> String {
        match ev {
            GameWorldSystemMessage::PlayerJoined { name, .. } => {
                format!("\"{}\" joined the game.", name.as_str())
            }
            GameWorldSystemMessage::PlayerLeft { name, .. } => {
                format!("\"{}\" left the game.", name.as_str())
            }
            GameWorldSystemMessage::Custom(msg) => msg.clone(),
        }
    }

    fn handle_action_feed(
        &mut self,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        ev: GameWorldActionFeed,
    ) {
        match ev {
            GameWorldActionFeed::Kill {
                killer,
                assists,
                victims,
                weapon,
                flags,
            } => {
                self.actionfeed
                    .msgs
                    .push_back(ActionFeed::Kill(ActionFeedKill {
                        killer: killer
                            .map(|killer| {
                                character_infos.get(&killer).map(|char| ActionFeedPlayer {
                                    name: char.name.clone(),
                                })
                            })
                            .flatten(),
                        assists: Vec::new(),
                        victims: victims
                            .iter()
                            .filter_map(|id| {
                                character_infos.get(id).map(|char| ActionFeedPlayer {
                                    name: char.name.clone(),
                                })
                            })
                            .collect(),
                        weapon: ActionFeedKillWeapon::World,
                        flags,
                    }));
            }
            GameWorldActionFeed::RaceFinish {
                characters: players,
                finish_time,
            } => {}
            GameWorldActionFeed::Custom(_) => todo!(),
        }
    }

    fn handle_events(&mut self, cur_time: &Duration, input: &mut RenderGameInput) {
        // keep scene active
        self.world_sound_scene.stay_active();
        // handle events
        for (stage_id, mut world) in input.events.worlds.drain() {
            for (_, ev) in world.events.drain() {
                match ev {
                    GameWorldEvent::Positioned(GameWorldPositionedEvent { ev, pos }) => {
                        match ev {
                            GameWorldEntityEvent::Character { ev, id } => match ev {
                                GameCharacterEvent::Sound(sound) => match sound {
                                    GameCharacterEventSound::WeaponSwitch { new_weapon } => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .by_type(new_weapon)
                                            .switch
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::NoAmmo { weapon } => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .by_type(weapon)
                                            .noammo
                                            [self.rng.random_int_in(0..=4) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::HammerFire => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .hammer
                                            .weapon
                                            .fire
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::GunFire => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .gun
                                            .fire
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::GrenadeFire => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .grenade
                                            .weapon
                                            .fire
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::LaserFire => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .laser
                                            .weapon
                                            .fire
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::ShotgunFire => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .shotgun
                                            .weapon
                                            .fire
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::GroundJump => {
                                        self.skin_container
                                            .get_or_default(&"TODO:".into())
                                            .sounds
                                            .ground_jump
                                            [self.rng.random_int_in(0..=7) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::AirJump => {
                                        self.skin_container
                                            .get_or_default(&"TODO:".into())
                                            .sounds
                                            .air_jump
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::Spawn => {
                                        self.skin_container
                                            .get_or_default(&"TODO:".into())
                                            .sounds
                                            .spawn
                                            [self.rng.random_int_in(0..=6) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::Death => {
                                        self.skin_container
                                            .get_or_default(&"TODO:".into())
                                            .sounds
                                            .death
                                            [self.rng.random_int_in(0..=3) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::HookHitPlayer => {
                                        self.hook_container
                                            .get_or_default(&"TODO:".into())
                                            .hit_player
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::HookHitHookable => {
                                        self.hook_container
                                            .get_or_default(&"TODO:".into())
                                            .hit_hookable
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::HookHitUnhookable => {
                                        self.hook_container
                                            .get_or_default(&"TODO:".into())
                                            .hit_unhookable
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::Pain { long } => {
                                        let sounds = &self
                                            .skin_container
                                            .get_or_default(&"TODO:".into())
                                            .sounds;
                                        let sounds = if long {
                                            sounds.pain_long.as_slice()
                                        } else {
                                            sounds.pain_short.as_slice()
                                        };
                                        sounds[self.rng.random_int_in(0..=sounds.len() as u64 - 1)
                                            as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::Hit { strong } => {
                                        let sounds = &self
                                            .skin_container
                                            .get_or_default(&"TODO:".into())
                                            .sounds;
                                        let hits = if strong {
                                            sounds.hit_strong.as_slice()
                                        } else {
                                            sounds.hit_weak.as_slice()
                                        };
                                        hits[self.rng.random_int_in(0..=hits.len() as u64 - 1)
                                            as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameCharacterEventSound::HammerHit { pos } => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .hammer
                                            .hits
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                },
                                GameCharacterEvent::Effect(eff) => match eff {
                                    GameCharacterEventEffect::Spawn => {
                                        Effects::new(&mut self.particles, *cur_time)
                                            .player_spawn(&pos);
                                    }
                                    GameCharacterEventEffect::Death => {
                                        Effects::new(&mut self.particles, *cur_time)
                                            .player_death(&pos, ColorRGBA::new(1.0, 1.0, 1.0, 1.0));
                                    }
                                    GameCharacterEventEffect::AirJump => {
                                        Effects::new(&mut self.particles, *cur_time).air_jump(&pos);
                                    }
                                    GameCharacterEventEffect::DamageIndicator { pos, vel } => {
                                        Effects::new(&mut self.particles, *cur_time)
                                            .damage_ind(&pos, &vel);
                                    }
                                    GameCharacterEventEffect::HammerHit { pos } => {
                                        Effects::new(&mut self.particles, *cur_time)
                                            .hammer_hit(&pos);
                                    }
                                },
                                GameCharacterEvent::Buff(ev) => match ev {
                                    GameBuffEvent::Ninja(ev) => match ev {
                                        GameBuffNinjaEvent::Sound(ev) => match ev {
                                            GameBuffNinjaEventSound::Spawn => {
                                                self.ninja_container
                                                    .get_or_default(&"TODO:".into())
                                                    .spawn
                                                    .play(SoundPlayProps::default())
                                                    .detatch();
                                            }
                                            GameBuffNinjaEventSound::Collect => {
                                                self.ninja_container
                                                    .get_or_default(&"TODO:".into())
                                                    .collect
                                                    .play(SoundPlayProps::default())
                                                    .detatch();
                                            }
                                            GameBuffNinjaEventSound::Attack => {
                                                self.ninja_container
                                                    .get_or_default(&"TODO:".into())
                                                    .attacks
                                                    [self.rng.random_int_in(0..=3) as usize]
                                                    .play(SoundPlayProps::default())
                                                    .detatch();
                                            }
                                            GameBuffNinjaEventSound::Hit => {
                                                self.ninja_container
                                                    .get_or_default(&"TODO:".into())
                                                    .hits
                                                    [self.rng.random_int_in(0..=3) as usize]
                                                    .play(SoundPlayProps::default())
                                                    .detatch();
                                            }
                                        },
                                        GameBuffNinjaEvent::Effect(ev) => match ev {},
                                    },
                                },
                                GameCharacterEvent::Debuff(ev) => match ev {
                                    GameDebuffEvent::Frozen(ev) => match ev {
                                        GameDebuffFrozenEvent::Sound(ev) => match ev {
                                            GameDebuffFrozenEventSound::Attack => {
                                                self.freeze_container
                                                    .get_or_default(&"TODO:".into())
                                                    .attack
                                                    .play(SoundPlayProps::default())
                                                    .detatch();
                                            }
                                        },
                                        GameDebuffFrozenEvent::Effect(ev) => match ev {},
                                    },
                                },
                            },
                            GameWorldEntityEvent::Grenade { ev, id } => match ev {
                                GameGrenadeEvent::Sound(ev) => match ev {
                                    GameGrenadeEventSound::Spawn => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .grenade
                                            .spawn
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameGrenadeEventSound::Collect => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .grenade
                                            .collect
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameGrenadeEventSound::Explosion => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .grenade
                                            .explosions
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                },
                                GameGrenadeEvent::Effect(ev) => match ev {
                                    GameGrenadeEventEffect::Explosion => {
                                        Effects::new(&mut self.particles, *cur_time)
                                            .explosion(&pos);
                                    }
                                },
                            },
                            GameWorldEntityEvent::Laser { id, ev } => match ev {
                                GameLaserEvent::Sound(ev) => match ev {
                                    GameLaserEventSound::Spawn => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .laser
                                            .spawn
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameLaserEventSound::Collect => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .laser
                                            .collect
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameLaserEventSound::Bounce => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .laser
                                            .bounces
                                            [self.rng.random_int_in(0..=2) as usize]
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                },
                                GameLaserEvent::Effect(ev) => match ev {},
                            },
                            GameWorldEntityEvent::Shotgun { id, ev } => match ev {
                                GameShotgunEvent::Sound(ev) => match ev {
                                    GameShotgunEventSound::Spawn => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .shotgun
                                            .spawn
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameShotgunEventSound::Collect => {
                                        self.weapon_container
                                            .get_or_default(&"TODO:".into())
                                            .shotgun
                                            .collect
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                },
                                GameShotgunEvent::Effect(ev) => match ev {},
                            },
                            GameWorldEntityEvent::Flag { id, ev } => match ev {
                                GameFlagEvent::Sound(ev) => match ev {
                                    GameFlagEventSound::Capture => {
                                        self.ctf_container
                                            .get_or_default(&"TODO:".into())
                                            .capture
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameFlagEventSound::Collect(ty) => match ty {
                                        // TODO: compare to local character team
                                        FlagType::Red => {
                                            self.ctf_container
                                                .get_or_default(&"TODO:".into())
                                                .collect_team
                                                .play(SoundPlayProps::default())
                                                .detatch();
                                        }
                                        FlagType::Blue => {
                                            self.ctf_container
                                                .get_or_default(&"TODO:".into())
                                                .collect_opponents
                                                .play(SoundPlayProps::default())
                                                .detatch();
                                        }
                                    },
                                    GameFlagEventSound::Drop => {
                                        self.ctf_container
                                            .get_or_default(&"TODO:".into())
                                            .drop
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                    GameFlagEventSound::Return => {
                                        self.ctf_container
                                            .get_or_default(&"TODO:".into())
                                            .return_sound
                                            .play(SoundPlayProps::default())
                                            .detatch();
                                    }
                                },
                                GameFlagEvent::Effect(ev) => match ev {},
                            },
                            GameWorldEntityEvent::Pickup { id, ev } => match ev {
                                GamePickupEvent::Heart(ev) => match ev {
                                    GamePickupHeartEvent::Sound(ev) => match ev {
                                        GamePickupHeartEventSound::Spawn => {
                                            self.game_container
                                                .get_or_default(&"TODO:".into())
                                                .heart
                                                .spawn
                                                .play(SoundPlayProps::default())
                                                .detatch();
                                        }
                                        GamePickupHeartEventSound::Collect => {
                                            self.game_container
                                                .get_or_default(&"TODO:".into())
                                                .heart
                                                .collects
                                                [self.rng.random_int_in(0..=1) as usize]
                                                .play(SoundPlayProps::default())
                                                .detatch();
                                        }
                                    },
                                    GamePickupHeartEvent::Effect(ev) => match ev {},
                                },
                                GamePickupEvent::Armor(ev) => match ev {
                                    GamePickupArmorEvent::Sound(ev) => match ev {
                                        GamePickupArmorEventSound::Spawn => {
                                            self.game_container
                                                .get_or_default(&"TODO:".into())
                                                .shield
                                                .spawn
                                                .play(SoundPlayProps::default())
                                                .detatch();
                                        }
                                        GamePickupArmorEventSound::Collect => {
                                            self.game_container
                                                .get_or_default(&"TODO:".into())
                                                .shield
                                                .collects
                                                [self.rng.random_int_in(0..=3) as usize]
                                                .play(SoundPlayProps::default())
                                                .detatch();
                                        }
                                    },
                                    GamePickupArmorEvent::Effect(ev) => match ev {},
                                },
                            },
                        }
                    }
                    GameWorldEvent::Global(ev) => match ev {
                        GameWorldGlobalEvent::System(ev) => {
                            self.chat.msgs.push_back(ServerMsg::System(MsgSystem {
                                msg: Self::convert_system_ev(ev),
                            }));
                        }
                        GameWorldGlobalEvent::ActionFeed(ev) => {
                            self.handle_action_feed(&input.character_infos, ev);
                        }
                    },
                }
            }
        }
    }

    fn from_net_msg(
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        msg: NetChatMsg,
    ) -> Option<ChatMsg> {
        if let Some(chat_info) = character_infos.get(&msg.player_id) {
            Some(ChatMsg {
                player: chat_info.name.clone(),
                clan: chat_info.clan.clone(),
                skin_name: chat_info.skin.clone(),
                msg: msg.msg,
                channel: ChatMsgPlayerChannel::from_net_msg(msg.channel),
            })
        } else {
            None
        }
    }

    fn handle_chat_msgs(&mut self, game: &mut RenderGameInput) {
        self.chat
            .msgs
            .extend(game.chat_msgs.drain(..).filter_map(|msg| {
                Self::from_net_msg(&game.character_infos, msg).map(|msg| ServerMsg::Chat(msg))
            }));
    }
}

impl RenderGameInterface for RenderGame {
    fn render(
        &mut self,
        config_map: &ConfigMap,
        cur_time: &Duration,
        mut input: RenderGameInput,
    ) -> RenderGameResult {
        // as a first step, update all containers
        self.skin_container
            .update(input.character_infos.values().map(|info| &*info.skin));

        let mut res = RenderGameResult::default();
        let map = self.map.try_get().unwrap();
        self.particles.update(cur_time, &map.data.collision);

        self.handle_chat_msgs(&mut input);
        self.handle_events(cur_time, &mut input);

        let player_count = input.players.len();
        if player_count == 1 {
            let mut render_info = input.players.drain(..).next().unwrap();
            let player_id = render_info.render_for_player.as_ref().map(|p| p.player_id);
            self.render_ingame(config_map, cur_time, &input, &render_info);
            self.backend_handle.consumble_multi_samples();
            let res_render = self.render_uis(cur_time, &input, &mut render_info);
            if let Some(player_id) = player_id {
                res.player_events.insert(player_id, res_render);
            }
        } else {
            let players_per_row = (player_count as f64).sqrt().ceil() as usize;
            let window_props = self.canvas_handle.window_props();

            let mut helper = self.helper.new();
            helper.extend(input.players.drain(..).enumerate().map(
                |(index, render_for_player_game)| {
                    let x = index % players_per_row;
                    let y = index / players_per_row;
                    let w_splitted = window_props.window_width as usize / players_per_row;
                    let mut h_splitted = window_props.window_height as usize / players_per_row;

                    if player_count <= (players_per_row * players_per_row) - players_per_row {
                        h_splitted = window_props.window_height as usize / (players_per_row - 1);
                    }

                    let (x, y, w, h) = (
                        (x * w_splitted) as i32,
                        (y * h_splitted) as i32,
                        w_splitted as u32,
                        h_splitted as u32,
                    );

                    (x, y, w, h, render_for_player_game)
                },
            ));

            for (x, y, w, h, render_for_player_game) in helper.iter() {
                self.canvas_handle.update_window_viewport(*x, *y, *w, *h);
                self.render_ingame(config_map, cur_time, &input, render_for_player_game);
            }
            self.backend_handle.consumble_multi_samples();
            for (x, y, w, h, render_for_player_game) in helper.iter_mut() {
                self.canvas_handle.update_window_viewport(*x, *y, *w, *h);
                let player_id = render_for_player_game
                    .render_for_player
                    .as_ref()
                    .map(|p| p.player_id);
                let res_render = self.render_uis(cur_time, &input, render_for_player_game);
                if let Some(player_id) = player_id {
                    res.player_events.insert(player_id, res_render);
                }
            }
            self.canvas_handle.reset_window_viewport();
        }
        self.particles.update_rates();

        res
    }

    fn continue_map_loading(&mut self, config: &ConfigDebug) -> bool {
        self.map.continue_loading(config).is_some()
    }
}
