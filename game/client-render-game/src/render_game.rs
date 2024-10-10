use std::{borrow::Borrow, collections::HashMap, num::NonZeroU32, sync::Arc, time::Duration};

use crate::{
    components::{
        cursor::{RenderCursor, RenderCursorPipe},
        game_objects::{GameObjectsRender, GameObjectsRenderPipe},
        hud::{RenderHud, RenderHudPipe},
        players::{PlayerRenderPipe, Players},
    },
    map::render_map_base::{ClientMapRender, RenderMapLoading},
};
use base_io::io::Io;
use client_containers::utils::{load_containers, RenderGameContainers};
use client_render::{
    actionfeed::render::{ActionfeedRender, ActionfeedRenderPipe},
    chat::render::{ChatRender, ChatRenderOptions, ChatRenderPipe},
    emote_wheel::render::{EmoteWheelRender, EmoteWheelRenderPipe},
    scoreboard::render::{ScoreboardRender, ScoreboardRenderPipe},
    vote::render::{VoteRender, VoteRenderPipe},
};
use client_render_base::{
    map::{
        map::RenderMap,
        render_pipe::{Camera, GameTimeInfo, RenderPipeline},
    },
    render::{
        effects::Effects,
        particle_manager::{ParticleGroup, ParticleManager},
    },
};
use client_types::{
    actionfeed::{Action, ActionInFeed, ActionKill, ActionPlayer},
    chat::{ChatMsg, ChatMsgPlayerChannel, MsgSystem, ServerMsg},
};
use client_ui::{
    chat::user_data::{ChatEvent, MsgInChat},
    emote_wheel::user_data::EmoteWheelEvent,
    vote::user_data::{VoteRenderData, VoteRenderPlayer, VoteRenderType},
};
use config::config::ConfigDebug;
use egui::Rect;
use game_config::config::{ConfigDummyScreenAnchor, ConfigMap};
use game_interface::{
    chat_commands::ChatCommands,
    events::{
        GameBuffEvent, GameBuffNinjaEvent, GameBuffNinjaEventSound, GameCharacterEvent,
        GameCharacterEventEffect, GameCharacterEventSound, GameDebuffEvent, GameDebuffFrozenEvent,
        GameDebuffFrozenEventSound, GameEvents, GameFlagEvent, GameFlagEventSound,
        GameGrenadeEvent, GameGrenadeEventEffect, GameGrenadeEventSound, GameLaserEvent,
        GameLaserEventSound, GamePickupArmorEvent, GamePickupArmorEventSound, GamePickupEvent,
        GamePickupHeartEvent, GamePickupHeartEventSound, GameShotgunEvent, GameShotgunEventSound,
        GameWorldAction, GameWorldEntityEvent, GameWorldEvent, GameWorldGlobalEvent,
        GameWorldPositionedEvent, GameWorldSystemMessage,
    },
    types::{
        flag::FlagType,
        game::{GameEntityId, GameTickType},
        network_string::NetworkReducedAsciiString,
        render::{
            character::{CharacterBuff, CharacterInfo, LocalCharacterRenderInfo},
            scoreboard::Scoreboard,
            stage::StageRenderInfo,
        },
    },
    votes::{VoteState, VoteType, Voted},
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle},
};
use graphics_types::rendering::ColorRgba;
use hashlink::LinkedHashMap;
use math::math::{vector::vec2, Rng, RngSlice};
use pool::{
    datatypes::{PoolBTreeMap, PoolLinkedHashMap, PoolLinkedHashSet, PoolVec, PoolVecDeque},
    pool::Pool,
    rc::PoolRc,
};
use rayon::ThreadPool;
use serde::{Deserialize, Serialize};
use shared_base::network::types::chat::NetChatMsg;
use sound::{
    commands::SoundSceneCreateProps, scene_object::SceneObject, sound::SoundManager,
    sound_listener::SoundListener, types::SoundPlayProps,
};
use ui_base::{font_data::UiFontData, ui::UiCreator};
use url::Url;

#[derive(Serialize, Deserialize)]
pub enum PlayerFeedbackEvent {
    Chat(ChatEvent),
    EmoteWheel(EmoteWheelEvent),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderGameCreateOptions {
    pub physics_group_name: NetworkReducedAsciiString<24>,
    pub resource_download_server: Option<Url>,
    pub fonts: Arc<UiFontData>,
    pub sound_props: SoundSceneCreateProps,
}

#[derive(Default, Serialize, Deserialize)]
pub struct RenderGameResult {
    pub player_events: LinkedHashMap<GameEntityId, Vec<PlayerFeedbackEvent>>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub enum RenderPlayerCameraMode {
    #[default]
    Default,
    AtPos(vec2),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderForPlayer {
    pub chat_info: Option<(String, Option<egui::RawInput>)>,
    pub emote_wheel_input: Option<Option<egui::RawInput>>,
    pub local_player_info: LocalCharacterRenderInfo,
    pub chat_show_all: bool,
    pub scoreboard_active: bool,

    pub zoom: f32,
    pub cam_mode: RenderPlayerCameraMode,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObservedAnchoredSize {
    pub width: NonZeroU32,
    pub height: NonZeroU32,
}

impl Default for ObservedAnchoredSize {
    fn default() -> Self {
        Self {
            width: 40.try_into().unwrap(),
            height: 40.try_into().unwrap(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ObservedDummyAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl From<ConfigDummyScreenAnchor> for ObservedDummyAnchor {
    fn from(value: ConfigDummyScreenAnchor) -> Self {
        match value {
            ConfigDummyScreenAnchor::TopLeft => Self::TopLeft,
            ConfigDummyScreenAnchor::TopRight => Self::TopRight,
            ConfigDummyScreenAnchor::BottomLeft => Self::BottomLeft,
            ConfigDummyScreenAnchor::BottomRight => Self::BottomRight,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ObservedPlayer {
    /// Player observes a dummy
    Dummy {
        player_id: GameEntityId,
        local_player_info: LocalCharacterRenderInfo,
        anchor: ObservedDummyAnchor,
    },
    /// The server allows to obverse a voted player.
    Vote { player_id: GameEntityId },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderGameForPlayer {
    pub render_for_player: RenderForPlayer,
    /// Players that this player observes.
    /// What that means is:
    /// - A mini screen for the dummy is requested.
    /// - A player is about to be voted (kicked or whatever).
    pub observed_players: PoolVec<ObservedPlayer>,
    /// For all anchored observed players, these are the size properties.
    pub observed_anchored_size_props: ObservedAnchoredSize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RenderGameSettings {
    pub spartial_sound: bool,
    pub sound_playback_speed: f64,
    /// For music from the map
    pub map_sound_volume: f64,
    /// For all the various sounds ingame
    pub ingame_sound_volume: f64,

    pub nameplates: bool,
    pub nameplate_own: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderGameInput {
    pub players: PoolLinkedHashMap<GameEntityId, RenderGameForPlayer>,
    pub dummies: PoolLinkedHashSet<GameEntityId>,
    /// the bool indicates if the events were generated on the client or
    /// from the server.
    pub events: PoolBTreeMap<GameTickType, (GameEvents, bool)>,
    pub chat_msgs: PoolVecDeque<NetChatMsg>,
    /// Vote state
    pub vote: Option<(PoolRc<VoteState>, Option<Voted>, Duration)>,

    pub character_infos: PoolLinkedHashMap<GameEntityId, CharacterInfo>,
    pub stages: PoolLinkedHashMap<GameEntityId, StageRenderInfo>,
    pub scoreboard_info: Option<Scoreboard>,

    pub game_time_info: GameTimeInfo,

    pub settings: RenderGameSettings,
}

type RenderPlayerHelper = (i32, i32, u32, u32, (GameEntityId, RenderGameForPlayer));

pub struct RenderGame {
    // containers
    containers: RenderGameContainers,

    // render components
    players: Players,
    render: GameObjectsRender,
    cursor_render: RenderCursor,
    chat: ChatRender,
    actionfeed: ActionfeedRender,
    scoreboard: ScoreboardRender,
    hud: RenderHud,
    particles: ParticleManager,
    emote_wheel: EmoteWheelRender,
    vote: VoteRender,

    // chat commands
    chat_commands: ChatCommands,

    last_event_monotonic_tick: Option<GameTickType>,

    // map
    map: ClientMapRender,
    physics_group_name: NetworkReducedAsciiString<24>,

    canvas_handle: GraphicsCanvasHandle,
    backend_handle: GraphicsBackendHandle,

    // helpers
    helper: Pool<Vec<RenderPlayerHelper>>,

    world_sound_scene: SceneObject,
    world_sound_listeners: HashMap<GameEntityId, SoundListener>,
    world_sound_listeners_pool: Pool<HashMap<GameEntityId, SoundListener>>,
    rng: Rng,
}

impl RenderGame {
    pub fn new(
        sound: &SoundManager,
        graphics: &Graphics,
        io: &Io,
        thread_pool: &Arc<ThreadPool>,
        cur_time: &Duration,
        map_file: Vec<u8>,
        config: &ConfigDebug,
        props: RenderGameCreateOptions,
    ) -> Self {
        let scene = sound.scene_handle.create(props.sound_props.clone());

        let physics_group_name = props.physics_group_name;
        let map = ClientMapRender::new(RenderMapLoading::new(
            thread_pool.clone(),
            map_file,
            props.resource_download_server,
            io.clone(),
            sound,
            props.sound_props,
            graphics,
            config,
        ));

        let resource_http_download_url = None;
        let resource_server_download_url = None;

        let containers = load_containers(
            io,
            thread_pool,
            resource_http_download_url,
            resource_server_download_url,
            graphics,
            sound,
            &scene,
        );

        let mut creator = UiCreator::default();
        creator.load_font(&props.fonts);

        let players = Players::new(graphics, &creator);
        let render = GameObjectsRender::new(cur_time, graphics);
        let cursor_render = RenderCursor::new(graphics);
        let hud = RenderHud::new(graphics, &creator);
        let particles = ParticleManager::new(graphics, cur_time);

        let chat = ChatRender::new(graphics, &creator);
        let actionfeed = ActionfeedRender::new(graphics, &creator);
        let scoreboard = ScoreboardRender::new(graphics, &creator);
        let emote_wheel = EmoteWheelRender::new(graphics, &creator);
        let vote = VoteRender::new(graphics, &creator);

        Self {
            // containers
            containers,

            // components
            players,
            render,
            cursor_render,
            chat,
            actionfeed,
            scoreboard,
            hud,
            particles,
            emote_wheel,
            vote,

            // chat commands
            chat_commands: Default::default(),

            last_event_monotonic_tick: None,

            map,
            physics_group_name,

            canvas_handle: graphics.canvas_handle.clone(),
            backend_handle: graphics.backend_handle.clone(),

            helper: Pool::with_capacity(1),

            world_sound_scene: scene,
            world_sound_listeners: Default::default(),
            world_sound_listeners_pool: Pool::with_capacity(2),
            rng: Rng::new(0),
        }
    }

    fn render_ingame(
        &mut self,

        config_map: &ConfigMap,
        cur_time: &Duration,

        render_info: &RenderGameInput,
        player_info: Option<(&GameEntityId, &RenderForPlayer)>,
    ) {
        let map = self.map.try_get().unwrap();

        let mut cam = Camera {
            pos: Default::default(),
            zoom: 1.0,
        };

        let character_info =
            player_info.and_then(|(player_id, _)| render_info.character_infos.get(player_id));

        let own_character_render_info = character_info
            .zip(player_info.map(|(id, _)| id))
            .and_then(|(c, player_id)| {
                c.stage_id
                    .and_then(|id| render_info.stages.get(&id).map(|p| (p, player_id)))
            })
            .and_then(|(s, player_id)| s.world.characters.get(player_id));

        let mut cur_anim_time = Duration::ZERO;
        if let (Some((_, local_render_info)), Some(character)) =
            (player_info, own_character_render_info)
        {
            cam.pos = character.lerped_pos;
            cam.zoom = local_render_info.zoom;
            cur_anim_time = RenderMap::calc_anim_time(
                render_info.game_time_info.ticks_per_second,
                character.animation_ticks_passed,
                &render_info.game_time_info.intra_tick_time,
            );
        }
        if let Some((_, p)) = player_info {
            cam.pos = match p.cam_mode {
                RenderPlayerCameraMode::Default => {
                    // don't change position, camera was set to the correct position above
                    cam.pos
                }
                RenderPlayerCameraMode::AtPos(pos) => {
                    // also update zoom
                    cam.zoom = p.zoom;
                    pos
                }
            };
        }

        let render_map = map;

        // map + ingame objects
        let mut render_pipe = RenderPipeline::new(
            &render_map.data.buffered_map.map_visual,
            &render_map.data.buffered_map,
            config_map,
            cur_time,
            &cur_anim_time,
            &cam,
            &mut self.containers.entities_container,
            character_info.map(|c| c.info.entities.borrow()),
            self.physics_group_name.as_str(),
            render_info.settings.map_sound_volume,
        );
        render_map.render.render_background(&mut render_pipe);
        self.particles.render_group(
            ParticleGroup::ProjectileTrail,
            &mut self.containers.particles_container,
            character_info.map(|c| c.info.particles.borrow()),
            &cam,
        );
        for (_, stage) in render_info
            .stages
            .iter()
            .filter(|(&stage_id, _)| character_info.and_then(|c| c.stage_id) != Some(stage_id))
            .chain(
                character_info
                    .and_then(|c| c.stage_id)
                    .and_then(|stage_id| render_info.stages.get_key_value(&stage_id))
                    .into_iter(),
            )
        {
            self.render.render(&mut GameObjectsRenderPipe {
                particle_manager: &mut self.particles,
                cur_time,
                game_time_info: &render_info.game_time_info,

                projectiles: &stage.world.projectiles,
                flags: &stage.world.ctf_flags,
                pickups: &stage.world.pickups,
                lasers: &stage.world.lasers,
                character_infos: &render_info.character_infos,

                ctf_container: &mut self.containers.ctf_container,
                game_container: &mut self.containers.game_container,
                ninja_container: &mut self.containers.ninja_container,
                weapon_container: &mut self.containers.weapon_container,

                camera: &cam,

                local_character_id: player_info.map(|p| p.0),
            });
            self.players.render(&mut PlayerRenderPipe {
                cur_time,
                game_time_info: &render_info.game_time_info,
                render_infos: &stage.world.characters,
                character_infos: &render_info.character_infos,

                particle_manager: &mut self.particles,

                skins: &mut self.containers.skin_container,
                ninjas: &mut self.containers.ninja_container,
                freezes: &mut self.containers.freeze_container,
                hooks: &mut self.containers.hook_container,
                weapons: &mut self.containers.weapon_container,
                emoticons: &mut self.containers.emoticons_container,

                collision: &render_map.data.collision,
                camera: &cam,

                own_character: player_info.map(|(player_id, _)| player_id),
            });
        }
        let mut render_pipe = RenderPipeline::new(
            &render_map.data.buffered_map.map_visual,
            &render_map.data.buffered_map,
            config_map,
            cur_time,
            &cur_anim_time,
            &cam,
            &mut self.containers.entities_container,
            character_info.map(|c| c.info.entities.borrow()),
            self.physics_group_name.as_str(),
            render_info.settings.map_sound_volume,
        );
        render_map.render.render_physics_layers(
            &mut render_pipe.base,
            &render_map.data.buffered_map.render.physics_render_layers,
        );
        render_map.render.render_foreground(&mut render_pipe);

        for (_, stage) in render_info.stages.iter() {
            self.players.render_nameplates(
                cur_time,
                &cam,
                &stage.world.characters,
                &render_info.character_infos,
                render_info.settings.nameplates,
                render_info.settings.nameplate_own,
                player_info.map(|(player_id, _)| player_id),
            );
        }

        self.particles.render_groups(
            ParticleGroup::Explosions,
            &mut self.containers.particles_container,
            character_info.map(|c| c.info.particles.borrow()),
            &cam,
        );
        // cursor
        if let Some(player) = own_character_render_info {
            self.cursor_render.render(&mut RenderCursorPipe {
                mouse_cursor: player.cursor_pos,
                weapon_container: &mut self.containers.weapon_container,
                weapon_key: character_info.map(|c| c.info.weapon.borrow()),
                cur_weapon: player.cur_weapon,
                is_ninja: player.buffs.contains_key(&CharacterBuff::Ninja),
                ninja_container: &mut self.containers.ninja_container,
                ninja_key: character_info.map(|c| c.info.ninja.borrow()),
            });
        }
    }

    /// render hud + uis: chat, scoreboard etc.
    #[must_use]
    fn render_uis(
        &mut self,

        cur_time: &Duration,

        render_info: &RenderGameInput,
        mut player_info: Option<(&GameEntityId, &mut RenderGameForPlayer)>,
        player_vote_rect: &mut Option<Rect>,
    ) -> Vec<PlayerFeedbackEvent> {
        let mut res: Vec<PlayerFeedbackEvent> = Default::default();
        // chat & emote wheel
        if let Some((player_id, player_render_info)) = player_info
            .as_mut()
            .map(|(id, p)| (id, &mut p.render_for_player))
        {
            let mut dummy_str: String = Default::default();
            let mut dummy_str_ref = &mut dummy_str;
            let mut dummy_state = &mut None;

            let chat_active =
                if let Some((chat_msg, chat_state)) = &mut player_render_info.chat_info {
                    dummy_str_ref = chat_msg;
                    dummy_state = chat_state;
                    true
                } else {
                    false
                };

            res.extend(
                self.chat
                    .render(&mut ChatRenderPipe {
                        cur_time,
                        msg: dummy_str_ref,
                        options: ChatRenderOptions {
                            is_chat_input_active: chat_active,
                            show_chat_history: player_render_info.chat_show_all,
                        },
                        input: dummy_state,
                        player_id,
                        skin_container: &mut self.containers.skin_container,
                        tee_render: &mut self.players.tee_renderer,
                    })
                    .into_iter()
                    .map(PlayerFeedbackEvent::Chat),
            );

            let character_info = render_info.character_infos.get(player_id);

            let mut dummy_input = &mut None;

            let wheel_active = if let Some(emote_input) = &mut player_render_info.emote_wheel_input
            {
                dummy_input = emote_input;
                true
            } else {
                false
            };

            if wheel_active {
                let default_key = self.containers.emoticons_container.default_key.clone();
                let skin_default_key = self.containers.skin_container.default_key.clone();
                res.extend(
                    self.emote_wheel
                        .render(&mut EmoteWheelRenderPipe {
                            cur_time,
                            input: dummy_input,
                            skin_container: &mut self.containers.skin_container,
                            emoticons_container: &mut self.containers.emoticons_container,
                            tee_render: &mut self.players.tee_renderer,
                            emoticons: character_info
                                .map(|c| c.info.emoticons.borrow())
                                .unwrap_or(&*default_key),
                            skin: character_info
                                .map(|c| c.info.skin.borrow())
                                .unwrap_or(&*skin_default_key),
                            skin_info: &character_info.map(|c| c.skin_info),
                        })
                        .into_iter()
                        .map(PlayerFeedbackEvent::EmoteWheel),
                );
            }
        }

        let character_info = player_info
            .as_ref()
            .and_then(|(player_id, _)| render_info.character_infos.get(player_id));

        // action feed
        self.actionfeed.render(&mut ActionfeedRenderPipe {
            cur_time,
            skin_container: &mut self.containers.skin_container,
            tee_render: &mut self.players.tee_renderer,
            weapon_container: &mut self.containers.weapon_container,
            toolkit_render: &self.players.toolkit_renderer,
            ninja_container: &mut self.containers.ninja_container,
        });

        // hud + scoreboard
        if let Some((player_id, local_render_info)) =
            player_info.map(|(player_id, p)| (player_id, &p.render_for_player))
        {
            let stage = render_info
                .character_infos
                .get(player_id)
                .and_then(|c| c.stage_id.and_then(|id| render_info.stages.get(&id)));
            let p = stage.and_then(|s| s.world.characters.get(player_id));
            self.hud.render(&mut RenderHudPipe {
                hud_container: &mut self.containers.hud_container,
                hud_key: character_info.map(|c| c.info.hud.borrow()),
                weapon_container: &mut self.containers.weapon_container,
                weapon_key: character_info.map(|c| c.info.weapon.borrow()),
                local_player_render_info: &local_render_info.local_player_info,
                cur_weapon: p.map(|c| c.cur_weapon).unwrap_or_default(),
                race_timer_counter: &p.map(|p| p.game_ticks_passed).unwrap_or_default(),
                ticks_per_second: &render_info.game_time_info.ticks_per_second,
                cur_time,
                game: stage.map(|s| &s.game),
                skin_container: &mut self.containers.skin_container,
                skin_renderer: &self.players.tee_renderer,
                ctf_container: &mut self.containers.ctf_container,
                character_infos: &render_info.character_infos,
            });
            if let Some(scoreboard_info) = local_render_info
                .scoreboard_active
                .then_some(())
                .and(render_info.scoreboard_info.as_ref())
            {
                // scoreboard after hud
                self.scoreboard.render(&mut ScoreboardRenderPipe {
                    cur_time,
                    scoreboard: scoreboard_info,
                    character_infos: &render_info.character_infos,
                    skin_container: &mut self.containers.skin_container,
                    tee_render: &mut self.players.tee_renderer,
                    flags_container: &mut self.containers.flags_container,
                });
            }
        }

        // current vote
        if let Some((vote, voted, remaining_time)) = &render_info.vote {
            if let Some(ty) = match &vote.vote {
                VoteType::Map(map) => Some(VoteRenderType::Map(map)),
                VoteType::VoteKickPlayer { voted_player_id }
                | VoteType::VoteSpecPlayer { voted_player_id } => render_info
                    .character_infos
                    .get(voted_player_id)
                    .map(|player| {
                        let vote_player = VoteRenderPlayer {
                            name: &player.info.name,
                            skin: player.info.skin.borrow(),
                            skin_info: &player.skin_info,
                        };
                        if matches!(vote.vote, VoteType::VoteKickPlayer { .. }) {
                            VoteRenderType::PlayerVoteKick(vote_player)
                        } else {
                            VoteRenderType::PlayerVoteSpec(vote_player)
                        }
                    }),
                VoteType::Misc(vote) => Some(VoteRenderType::Misc(vote)),
            } {
                *player_vote_rect = self.vote.render(&mut VoteRenderPipe {
                    cur_time,
                    skin_container: &mut self.containers.skin_container,
                    tee_render: &mut self.players.tee_renderer,
                    vote_data: VoteRenderData {
                        ty,
                        data: vote,
                        remaining_time,
                        voted: *voted,
                    },
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
    fn continue_map_loading(&mut self) -> bool;
    fn set_chat_commands(&mut self, chat_commands: ChatCommands);
    /// Clear all rendering state (like particles, sounds etc.)
    fn clear_render_state(&mut self);
    /// Render sound for an off-air scene.
    /// If the game scene is not off-air,
    /// it will throw errors in the sound backend.
    fn render_offair_sound(&mut self, samples: u32);
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
            GameWorldSystemMessage::Custom(msg) => msg.to_string(),
        }
    }

    fn handle_action_feed(
        &mut self,
        cur_time: &Duration,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        ev: GameWorldAction,
    ) {
        match ev {
            GameWorldAction::Kill {
                killer,
                assists,
                victims,
                weapon,
                flags,
            } => {
                self.actionfeed.msgs.push_back(ActionInFeed {
                    action: Action::Kill(ActionKill {
                        killer: killer.and_then(|killer| {
                            character_infos.get(&killer).map(|char| ActionPlayer {
                                name: char.info.name.to_string(),
                                skin: char.info.skin.clone().into(),
                                skin_info: char.skin_info,
                                weapon: char.info.weapon.clone().into(),
                            })
                        }),
                        assists: assists
                            .iter()
                            .filter_map(|id| {
                                character_infos.get(id).map(|char| ActionPlayer {
                                    name: char.info.name.to_string(),
                                    skin: char.info.skin.clone().into(),
                                    skin_info: char.skin_info,
                                    weapon: char.info.weapon.clone().into(),
                                })
                            })
                            .collect(),
                        victims: victims
                            .iter()
                            .filter_map(|id| {
                                character_infos.get(id).map(|char| ActionPlayer {
                                    name: char.info.name.to_string(),
                                    skin: char.info.skin.clone().into(),
                                    skin_info: char.skin_info,
                                    weapon: char.info.weapon.clone().into(),
                                })
                            })
                            .collect(),
                        weapon,
                        flags,
                    }),
                    add_time: *cur_time,
                });
            }
            GameWorldAction::RaceFinish {
                character,
                finish_time,
            } => {
                if let Some(c) = character_infos.get(&character) {
                    self.actionfeed.msgs.push_back(ActionInFeed {
                        action: Action::RaceFinish {
                            player: ActionPlayer {
                                name: c.info.name.to_string(),
                                skin: c.info.skin.clone().into(),
                                skin_info: c.skin_info,
                                weapon: c.info.weapon.clone().into(),
                            },
                            finish_time,
                        },
                        add_time: *cur_time,
                    });
                }
            }
            GameWorldAction::RaceTeamFinish {
                characters,
                team_name,
                finish_time,
            } => {
                self.actionfeed.msgs.push_back(ActionInFeed {
                    action: Action::RaceTeamFinish {
                        players: characters
                            .iter()
                            .filter_map(|c| {
                                character_infos.get(c).map(|c| ActionPlayer {
                                    name: c.info.name.to_string(),
                                    skin: c.info.skin.clone().into(),
                                    skin_info: c.skin_info,
                                    weapon: c.info.weapon.clone().into(),
                                })
                            })
                            .collect(),
                        team_name: team_name.to_string(),
                        finish_time,
                    },
                    add_time: *cur_time,
                });
            }
            GameWorldAction::Custom(_) => todo!(),
        }
    }

    fn handle_character_event(
        &mut self,
        cur_time: &Duration,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        settings: &RenderGameSettings,
        pos: vec2,
        ev: GameCharacterEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameCharacterEvent::Sound(sound) => match sound {
                GameCharacterEventSound::WeaponSwitch { new_weapon } => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .by_type(new_weapon)
                        .switch
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::NoAmmo { weapon } => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .by_type(weapon)
                        .noammo
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HammerFire => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .hammer
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::GunFire => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .gun
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::GrenadeFire => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::LaserFire => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::ShotgunFire => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .shotgun
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::GroundJump => {
                    self.containers
                        .skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .ground_jump
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::AirJump => {
                    self.containers
                        .skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .air_jump
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Spawn => {
                    self.containers
                        .skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .spawn
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Death => {
                    self.containers
                        .skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .death
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HookHitPlayer => {
                    self.containers
                        .hook_container
                        .get_or_default_opt(info.map(|i| &i.hook))
                        .hit_player
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HookHitHookable => {
                    self.containers
                        .hook_container
                        .get_or_default_opt(info.map(|i| &i.hook))
                        .hit_hookable
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HookHitUnhookable => {
                    self.containers
                        .hook_container
                        .get_or_default_opt(info.map(|i| &i.hook))
                        .hit_unhookable
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Pain { long } => {
                    let sounds = &self
                        .containers
                        .skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds;
                    let sounds = if long {
                        sounds.pain_long.as_slice()
                    } else {
                        sounds.pain_short.as_slice()
                    };
                    sounds
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Hit { strong } => {
                    let sounds = &self
                        .containers
                        .skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds;
                    let hits = if strong {
                        sounds.hit_strong.as_slice()
                    } else {
                        sounds.hit_weak.as_slice()
                    };
                    hits.random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HammerHit => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .hammer
                        .hits
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
            },
            GameCharacterEvent::Effect(eff) => match eff {
                GameCharacterEventEffect::Spawn => {
                    Effects::new(&mut self.particles, *cur_time).player_spawn(&pos);
                }
                GameCharacterEventEffect::Death => {
                    Effects::new(&mut self.particles, *cur_time)
                        .player_death(&pos, ColorRgba::new(1.0, 1.0, 1.0, 1.0));
                }
                GameCharacterEventEffect::AirJump => {
                    Effects::new(&mut self.particles, *cur_time).air_jump(&pos);
                }
                GameCharacterEventEffect::DamageIndicator { vel } => {
                    Effects::new(&mut self.particles, *cur_time).damage_ind(&pos, &vel);
                }
                GameCharacterEventEffect::HammerHit => {
                    Effects::new(&mut self.particles, *cur_time).hammer_hit(&pos);
                }
            },
            GameCharacterEvent::Buff(ev) => match ev {
                GameBuffEvent::Ninja(ev) => match ev {
                    GameBuffNinjaEvent::Sound(ev) => match ev {
                        GameBuffNinjaEventSound::Spawn => {
                            self.containers
                                .ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .spawn
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(settings.spartial_sound)
                                        .with_playback_speed(settings.sound_playback_speed)
                                        .with_volume(settings.ingame_sound_volume),
                                )
                                .detatch();
                        }
                        GameBuffNinjaEventSound::Collect => {
                            self.containers
                                .ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .collect
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(settings.spartial_sound)
                                        .with_playback_speed(settings.sound_playback_speed)
                                        .with_volume(settings.ingame_sound_volume),
                                )
                                .detatch();
                        }
                        GameBuffNinjaEventSound::Attack => {
                            self.containers
                                .ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .attacks
                                .random_entry(&mut self.rng)
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(settings.spartial_sound)
                                        .with_playback_speed(settings.sound_playback_speed)
                                        .with_volume(settings.ingame_sound_volume),
                                )
                                .detatch();
                        }
                        GameBuffNinjaEventSound::Hit => {
                            self.containers
                                .ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .hits
                                .random_entry(&mut self.rng)
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(settings.spartial_sound)
                                        .with_playback_speed(settings.sound_playback_speed)
                                        .with_volume(settings.ingame_sound_volume),
                                )
                                .detatch();
                        }
                    },
                },
            },
            GameCharacterEvent::Debuff(ev) => match ev {
                GameDebuffEvent::Frozen(ev) => match ev {
                    GameDebuffFrozenEvent::Sound(ev) => match ev {
                        GameDebuffFrozenEventSound::Attack => {
                            self.containers
                                .freeze_container
                                .get_or_default_opt(info.map(|i| &i.freeze))
                                .attacks
                                .random_entry(&mut self.rng)
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(settings.spartial_sound)
                                        .with_playback_speed(settings.sound_playback_speed)
                                        .with_volume(settings.ingame_sound_volume),
                                )
                                .detatch();
                        }
                    },
                },
            },
        }
    }

    fn handle_grenade_event(
        &mut self,
        cur_time: &Duration,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        settings: &RenderGameSettings,
        pos: vec2,
        ev: GameGrenadeEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameGrenadeEvent::Sound(ev) => match ev {
                GameGrenadeEventSound::Spawn => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .spawn
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameGrenadeEventSound::Collect => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .collect
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameGrenadeEventSound::Explosion => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .explosions
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
            },
            GameGrenadeEvent::Effect(ev) => match ev {
                GameGrenadeEventEffect::Explosion => {
                    Effects::new(&mut self.particles, *cur_time).explosion(&pos);
                }
            },
        }
    }

    fn handle_laser_event(
        &mut self,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        settings: &RenderGameSettings,
        pos: vec2,
        ev: GameLaserEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameLaserEvent::Sound(ev) => match ev {
                GameLaserEventSound::Spawn => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .spawn
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameLaserEventSound::Collect => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .collect
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameLaserEventSound::Bounce => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .bounces
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
            },
        }
    }

    fn handle_shotgun_event(
        &mut self,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        settings: &RenderGameSettings,
        pos: vec2,
        ev: GameShotgunEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameShotgunEvent::Sound(ev) => match ev {
                GameShotgunEventSound::Spawn => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .shotgun
                        .spawn
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameShotgunEventSound::Collect => {
                    self.containers
                        .weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .shotgun
                        .collect
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
            },
        }
    }

    fn handle_flag_event(
        &mut self,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        settings: &RenderGameSettings,
        pos: vec2,
        ev: GameFlagEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameFlagEvent::Sound(ev) => match ev {
                GameFlagEventSound::Capture => {
                    self.containers
                        .ctf_container
                        .get_or_default_opt(info.map(|i| &i.ctf))
                        .capture
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameFlagEventSound::Collect(ty) => match ty {
                    // TODO: compare to local character side
                    FlagType::Red => {
                        self.containers
                            .ctf_container
                            .get_or_default_opt(info.map(|i| &i.ctf))
                            .collect_friendly
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(settings.spartial_sound)
                                    .with_playback_speed(settings.sound_playback_speed)
                                    .with_volume(settings.ingame_sound_volume),
                            )
                            .detatch();
                    }
                    FlagType::Blue => {
                        self.containers
                            .ctf_container
                            .get_or_default_opt(info.map(|i| &i.ctf))
                            .collect_opponents
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(settings.spartial_sound)
                                    .with_playback_speed(settings.sound_playback_speed)
                                    .with_volume(settings.ingame_sound_volume),
                            )
                            .detatch();
                    }
                },
                GameFlagEventSound::Drop => {
                    self.containers
                        .ctf_container
                        .get_or_default_opt(info.map(|i| &i.ctf))
                        .drop
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
                GameFlagEventSound::Return => {
                    self.containers
                        .ctf_container
                        .get_or_default_opt(info.map(|i| &i.ctf))
                        .return_sound
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(settings.spartial_sound)
                                .with_playback_speed(settings.sound_playback_speed)
                                .with_volume(settings.ingame_sound_volume),
                        )
                        .detatch();
                }
            },
        }
    }

    fn handle_pickup_event(
        &mut self,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        settings: &RenderGameSettings,
        pos: vec2,
        ev: GamePickupEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| character_infos.get(&id).map(|c| &c.info));
        match ev {
            GamePickupEvent::Heart(ev) => match ev {
                GamePickupHeartEvent::Sound(ev) => match ev {
                    GamePickupHeartEventSound::Spawn => {
                        self.containers
                            .game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .heart
                            .spawn
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(settings.spartial_sound)
                                    .with_playback_speed(settings.sound_playback_speed)
                                    .with_volume(settings.ingame_sound_volume),
                            )
                            .detatch();
                    }
                    GamePickupHeartEventSound::Collect => {
                        self.containers
                            .game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .heart
                            .collects
                            .random_entry(&mut self.rng)
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(settings.spartial_sound)
                                    .with_playback_speed(settings.sound_playback_speed)
                                    .with_volume(settings.ingame_sound_volume),
                            )
                            .detatch();
                    }
                },
            },
            GamePickupEvent::Armor(ev) => match ev {
                GamePickupArmorEvent::Sound(ev) => match ev {
                    GamePickupArmorEventSound::Spawn => {
                        self.containers
                            .game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .shield
                            .spawn
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(settings.spartial_sound)
                                    .with_playback_speed(settings.sound_playback_speed)
                                    .with_volume(settings.ingame_sound_volume),
                            )
                            .detatch();
                    }
                    GamePickupArmorEventSound::Collect => {
                        self.containers
                            .game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .shield
                            .collects
                            .random_entry(&mut self.rng)
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(settings.spartial_sound)
                                    .with_playback_speed(settings.sound_playback_speed)
                                    .with_volume(settings.ingame_sound_volume),
                            )
                            .detatch();
                    }
                },
            },
        }
    }

    fn handle_positioned_event(
        &mut self,
        is_prediction: bool,
        event_tick_unknown: bool,
        cur_time: &Duration,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        local_players: &PoolLinkedHashMap<GameEntityId, RenderGameForPlayer>,
        local_dummies: &PoolLinkedHashSet<GameEntityId>,
        settings: &RenderGameSettings,
        GameWorldPositionedEvent { owner_id, ev, pos }: GameWorldPositionedEvent,
    ) {
        if is_prediction
            && !owner_id
                .is_some_and(|id| local_players.contains_key(&id) || local_dummies.contains(&id))
            || !is_prediction
                && !event_tick_unknown
                && owner_id.is_some_and(|id| {
                    local_players.contains_key(&id) || local_dummies.contains(&id)
                })
        {
            return;
        }
        match ev {
            GameWorldEntityEvent::Character { ev } => {
                self.handle_character_event(cur_time, character_infos, settings, pos, ev, owner_id);
            }
            GameWorldEntityEvent::Grenade { ev } => {
                self.handle_grenade_event(cur_time, character_infos, settings, pos, ev, owner_id);
            }
            GameWorldEntityEvent::Laser { ev } => {
                self.handle_laser_event(character_infos, settings, pos, ev, owner_id);
            }
            GameWorldEntityEvent::Shotgun { ev } => {
                self.handle_shotgun_event(character_infos, settings, pos, ev, owner_id);
            }
            GameWorldEntityEvent::Flag { ev } => {
                self.handle_flag_event(character_infos, settings, pos, ev, owner_id);
            }
            GameWorldEntityEvent::Pickup { ev } => {
                self.handle_pickup_event(character_infos, settings, pos, ev, owner_id);
            }
        }
    }

    fn handle_events(&mut self, cur_time: &Duration, input: &mut RenderGameInput) {
        // handle events
        for (monotonic_tick, (events, by_prediction)) in input.events.iter_mut() {
            let event_tick_unknown = !self
                .last_event_monotonic_tick
                .is_some_and(|tick| tick >= *monotonic_tick);
            for (stage_id, mut world) in events.worlds.drain() {
                if !input.stages.contains_key(&stage_id) {
                    continue;
                }
                for (_, ev) in world.events.drain() {
                    match ev {
                        GameWorldEvent::Positioned(ev) => self.handle_positioned_event(
                            *by_prediction,
                            event_tick_unknown,
                            cur_time,
                            &input.character_infos,
                            &input.players,
                            &input.dummies,
                            &input.settings,
                            ev,
                        ),
                        GameWorldEvent::Global(ev) => {
                            // don't rely on prediction for global events.
                            if !*by_prediction {
                                match ev {
                                    GameWorldGlobalEvent::System(ev) => {
                                        self.chat.msgs.push_back(MsgInChat {
                                            msg: ServerMsg::System(MsgSystem {
                                                msg: Self::convert_system_ev(ev),
                                            }),
                                            add_time: *cur_time,
                                        })
                                    }
                                    GameWorldGlobalEvent::Action(ev) => {
                                        self.handle_action_feed(
                                            cur_time,
                                            &input.character_infos,
                                            ev,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            self.last_event_monotonic_tick = self
                .last_event_monotonic_tick
                .map(|tick| tick.max(*monotonic_tick))
                .or(Some(*monotonic_tick));
        }
        input.events.clear();
    }

    fn from_net_msg(
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        msg: NetChatMsg,
    ) -> Option<ChatMsg> {
        if let Some(chat_info) = character_infos.get(&msg.player_id) {
            Some(ChatMsg {
                player: chat_info.info.name.to_string(),
                clan: chat_info.info.clan.to_string(),
                skin_name: chat_info.info.skin.clone().into(),
                skin_info: chat_info.skin_info,
                msg: msg.msg,
                channel: ChatMsgPlayerChannel::from_net_msg(msg.channel),
            })
        } else {
            None
        }
    }

    fn handle_chat_msgs(&mut self, cur_time: &Duration, game: &mut RenderGameInput) {
        let it = game.chat_msgs.drain(..).filter_map(|msg| {
            Self::from_net_msg(&game.character_infos, msg).map(|msg| MsgInChat {
                msg: ServerMsg::Chat(msg),
                add_time: *cur_time,
            })
        });
        for msg in it {
            // push_back is intentionally used over extend, so msgs are
            // only mutable accessed if a new msg is actually added
            self.chat.msgs.push_back(msg);
        }
    }

    fn calc_players_per_row(player_count: usize) -> usize {
        (player_count as f64).sqrt().ceil() as usize
    }

    fn player_render_area(
        index: usize,
        width: u32,
        height: u32,
        players_per_row: usize,
        player_count: usize,
    ) -> (i32, i32, u32, u32) {
        let x = index % players_per_row;
        let y = index / players_per_row;
        let w_splitted = width as usize / players_per_row;
        let mut h_splitted = height as usize / players_per_row;

        if player_count <= (players_per_row * players_per_row) - players_per_row {
            h_splitted = height as usize / (players_per_row - 1);
        }

        let (x, y, w, h) = (
            (x * w_splitted) as i32,
            (y * h_splitted) as i32,
            w_splitted as u32,
            h_splitted as u32,
        );

        (x, y, w.max(1), h.max(1))
    }

    fn render_observers(
        &mut self,
        observed_players: &[ObservedPlayer],
        anchored_size: &ObservedAnchoredSize,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        config_map: &ConfigMap,
        cur_time: &Duration,
        input: &RenderGameInput,
        player_vote_rect: Option<Rect>,
    ) {
        let (top_left, top_right, bottom_left, bottom_right) = {
            let mut top_left = 0;
            let mut top_right = 0;
            let mut bottom_left = 0;
            let mut bottom_right = 0;
            observed_players.iter().for_each(|d| {
                if let ObservedPlayer::Dummy { anchor, .. } = d {
                    match anchor {
                        ObservedDummyAnchor::TopLeft => {
                            top_left += 1;
                        }
                        ObservedDummyAnchor::TopRight => {
                            top_right += 1;
                        }
                        ObservedDummyAnchor::BottomLeft => {
                            bottom_left += 1;
                        }
                        ObservedDummyAnchor::BottomRight => {
                            bottom_right += 1;
                        }
                    }
                }
            });
            (top_left, top_right, bottom_left, bottom_right)
        };
        for (index, observed_player) in observed_players.iter().enumerate() {
            match observed_player {
                ObservedPlayer::Dummy {
                    player_id,
                    local_player_info,
                    anchor,
                } => {
                    let player_count = match anchor {
                        ObservedDummyAnchor::TopLeft => top_left,
                        ObservedDummyAnchor::TopRight => top_right,
                        ObservedDummyAnchor::BottomLeft => bottom_left,
                        ObservedDummyAnchor::BottomRight => bottom_right,
                    };
                    let players_per_row = Self::calc_players_per_row(player_count);
                    let (px, py, pw, ph) = Self::player_render_area(
                        index,
                        ((w / 2) * anchored_size.width.get()) / 100,
                        ((h / 2) * anchored_size.height.get()) / 100,
                        players_per_row,
                        player_count,
                    );
                    let (off_x, off_y) = match anchor {
                        ObservedDummyAnchor::TopLeft => (px, py),
                        ObservedDummyAnchor::TopRight => (w as i32 - (pw as i32 - px), py),
                        ObservedDummyAnchor::BottomLeft => (px, h as i32 - (ph as i32 - py)),
                        ObservedDummyAnchor::BottomRight => {
                            (w as i32 - (pw as i32 - px), h as i32 - (ph as i32 - py))
                        }
                    };
                    self.canvas_handle
                        .update_window_viewport(x + off_x, y + off_y, pw, ph);
                    self.render_ingame(
                        config_map,
                        cur_time,
                        input,
                        Some((
                            player_id,
                            &RenderForPlayer {
                                chat_info: None,
                                emote_wheel_input: None,
                                local_player_info: *local_player_info,
                                chat_show_all: false,
                                scoreboard_active: false,

                                zoom: 1.0,
                                cam_mode: RenderPlayerCameraMode::Default,
                            },
                        )),
                    );
                }
                ObservedPlayer::Vote { player_id } => {
                    if let Some((_, player_vote_rect)) = input
                        .character_infos
                        .get(player_id)
                        .and_then(|c| {
                            c.stage_id.and_then(|stage_id| {
                                input
                                    .stages
                                    .get(&stage_id)
                                    .and_then(|stage| stage.world.characters.get(player_id))
                            })
                        })
                        .zip(player_vote_rect)
                    {
                        let ppp = self.canvas_handle.window_pixels_per_point();
                        let off_x = (player_vote_rect.min.x * ppp).round() as i32;
                        let off_y = (player_vote_rect.min.y * ppp).round() as i32;
                        let pw = (player_vote_rect.width() * ppp).round() as u32;
                        let ph = (player_vote_rect.height() * ppp).round() as u32;
                        self.canvas_handle
                            .update_window_viewport(x + off_x, y + off_y, pw, ph);
                        self.render_ingame(
                            config_map,
                            cur_time,
                            input,
                            Some((
                                player_id,
                                &RenderForPlayer {
                                    chat_info: None,
                                    emote_wheel_input: None,
                                    local_player_info: LocalCharacterRenderInfo {
                                        health: 10,
                                        armor: 10,
                                        ammo_of_weapon: None,
                                    },
                                    chat_show_all: false,
                                    scoreboard_active: false,

                                    zoom: 1.0,
                                    cam_mode: RenderPlayerCameraMode::Default,
                                },
                            )),
                        );
                    }
                }
            }
        }
    }

    fn update_containers(
        &mut self,
        cur_time: &Duration,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
    ) {
        self.containers.skin_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.weapon_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.hook_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.ctf_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.ninja_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.freeze_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.entities_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.hud_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.emoticons_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.particles_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
        self.containers.game_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            character_infos.values().map(|info| info.info.skin.borrow()),
        );
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
        self.update_containers(cur_time, &input.character_infos);

        // keep scene active
        self.world_sound_scene.stay_active();

        let mut res = RenderGameResult::default();
        let map = self.map.try_get().unwrap();
        self.particles.update(cur_time, &map.data.collision);

        self.handle_chat_msgs(cur_time, &mut input);
        self.handle_events(cur_time, &mut input);

        let mut next_sound_listeners = self.world_sound_listeners_pool.new();
        std::mem::swap(&mut *next_sound_listeners, &mut self.world_sound_listeners);
        for player_id in input.players.keys() {
            if let Some(c) = input.character_infos.get(player_id).and_then(|c| {
                c.stage_id
                    .and_then(|id| input.stages.get(&id))
                    .and_then(|s| s.world.characters.get(player_id))
            }) {
                if let Some(listener) = next_sound_listeners.remove(player_id) {
                    self.world_sound_listeners
                        .entry(*player_id)
                        .or_insert(listener)
                } else {
                    self.world_sound_listeners.entry(*player_id).or_insert(
                        self.world_sound_scene
                            .sound_listener_handle
                            .create(c.lerped_pos),
                    )
                }
                .update(c.lerped_pos);
            }
        }

        let player_count = input.players.len();
        if player_count == 0 {
            self.render_ingame(config_map, cur_time, &input, None);
            self.backend_handle.consumble_multi_samples();
            let _ = self.render_uis(cur_time, &input, None, &mut None);
        } else {
            let players_per_row = Self::calc_players_per_row(player_count);
            let window_props = self.canvas_handle.window_props();

            let mut helper = self.helper.new();
            let has_viewport_updates = if player_count == 1 {
                let (player_id, render_for_player_game) = input.players.drain().next().unwrap();
                let has_observed_players = !render_for_player_game.observed_players.is_empty();
                helper.push((
                    0,
                    0,
                    window_props.window_width,
                    window_props.window_height,
                    (player_id, render_for_player_game),
                ));
                has_observed_players
            } else {
                helper.extend(input.players.drain().enumerate().map(
                    |(index, (player_id, render_for_player_game))| {
                        let (x, y, w, h) = Self::player_render_area(
                            index,
                            window_props.window_width,
                            window_props.window_height,
                            players_per_row,
                            player_count,
                        );
                        (x, y, w, h, (player_id, render_for_player_game))
                    },
                ));
                true
            };

            for (x, y, w, h, (player_id, render_for_player_game)) in helper.iter() {
                if has_viewport_updates {
                    self.canvas_handle.update_window_viewport(*x, *y, *w, *h);
                }
                self.render_ingame(
                    config_map,
                    cur_time,
                    &input,
                    Some((player_id, &render_for_player_game.render_for_player)),
                );
            }
            self.backend_handle.consumble_multi_samples();
            for (x, y, w, h, (player_id, render_for_player_game)) in helper.iter_mut() {
                if has_viewport_updates {
                    self.canvas_handle.update_window_viewport(*x, *y, *w, *h);
                }
                let mut player_vote_rect = None;
                let res_render = self.render_uis(
                    cur_time,
                    &input,
                    Some((player_id, render_for_player_game)),
                    &mut player_vote_rect,
                );
                res.player_events.insert(*player_id, res_render);

                // render observers
                self.render_observers(
                    &render_for_player_game.observed_players,
                    &render_for_player_game.observed_anchored_size_props,
                    *x,
                    *y,
                    *w,
                    *h,
                    config_map,
                    cur_time,
                    &input,
                    player_vote_rect,
                );
            }
            if has_viewport_updates {
                self.canvas_handle.reset_window_viewport();
            }
        }
        self.particles.update_rates();

        res
    }

    fn continue_map_loading(&mut self) -> bool {
        self.map.continue_loading().is_some()
    }

    fn set_chat_commands(&mut self, chat_commands: ChatCommands) {
        self.chat_commands = chat_commands
    }

    fn clear_render_state(&mut self) {
        self.particles.reset();
        self.world_sound_scene.stop_detatched_sounds();
        self.last_event_monotonic_tick = None;
        self.chat.msgs.clear();
        self.actionfeed.msgs.clear();
    }

    fn render_offair_sound(&mut self, samples: u32) {
        self.world_sound_scene.process_off_air(samples);
    }
}
