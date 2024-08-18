use std::{borrow::Borrow, collections::HashMap, num::NonZeroU32, sync::Arc, time::Duration};

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
use base_io::io::Io;
use client_containers::{
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
    emote_wheel::render::{EmoteWheelRender, EmoteWheelRenderPipe},
    scoreboard::render::{ScoreboardRender, ScoreboardRenderPipe},
    vote::render::{VoteRender, VoteRenderPipe},
};
use client_render_base::map::{
    map::RenderMap,
    render_pipe::{Camera, GameTimeInfo, RenderPipeline},
};
use client_types::{
    actionfeed::{ActionFeed, ActionFeedKill, ActionFeedKillWeapon, ActionFeedPlayer},
    chat::{ChatMsg, ChatMsgPlayerChannel, MsgSystem, ServerMsg},
};
use client_ui::{
    chat::user_data::{ChatEvent, MsgInChat},
    emote_wheel::user_data::EmoteWheelEvent,
    vote::user_data::{VoteRenderData, VoteRenderPlayer, VoteRenderType},
};
use config::config::{ConfigDebug, ConfigEngine};
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
        GameWorldActionFeed, GameWorldEntityEvent, GameWorldEvent, GameWorldGlobalEvent,
        GameWorldPositionedEvent, GameWorldSystemMessage,
    },
    types::{
        emoticons::EmoticonType,
        flag::FlagType,
        game::GameEntityId,
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
use graphics_types::rendering::ColorRGBA;
use hashlink::LinkedHashMap;
use math::math::{vector::vec2, Rng, RngSlice};
use pool::{
    datatypes::{PoolLinkedHashMap, PoolVec, PoolVecDeque},
    pool::Pool,
    rc::PoolRc,
};
use rayon::ThreadPool;
use serde::{Deserialize, Serialize};
use shared_base::network::types::chat::NetChatMsg;
use sound::{
    scene_object::SceneObject, sound::SoundManager, sound_listener::SoundListener,
    types::SoundPlayProps,
};
use ui_base::{font_data::UiFontData, ui::UiCreator};
use url::Url;

#[derive(Serialize, Deserialize)]
pub enum PlayerFeedbackEvent {
    Chat(ChatEvent),
    EmoteWheel(EmoteWheelEvent),
}

#[derive(Default, Serialize, Deserialize)]
pub struct RenderGameResult {
    pub player_events: LinkedHashMap<GameEntityId, Vec<PlayerFeedbackEvent>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderForPlayer {
    pub chat_info: Option<(String, Option<egui::RawInput>)>,
    pub emote_wheel_info: Option<(Option<EmoticonType>, Option<egui::RawInput>)>,
    pub local_player_info: LocalCharacterRenderInfo,
    pub chat_show_all: bool,
    pub scoreboard_active: bool,
    pub player_id: GameEntityId,
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct RenderGameSettings {
    pub spartial_sound: bool,
    pub nameplates: bool,
    pub nameplate_own: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenderGameInput {
    pub players: PoolVec<RenderGameForPlayer>,
    pub events: Option<GameEvents>,
    pub chat_msgs: PoolVecDeque<NetChatMsg>,
    /// Vote state
    pub vote: Option<(PoolRc<VoteState>, Option<Voted>, Duration)>,

    pub character_infos: PoolLinkedHashMap<GameEntityId, CharacterInfo>,
    pub stages: PoolLinkedHashMap<GameEntityId, StageRenderInfo>,
    pub scoreboard_info: Option<Scoreboard>,

    pub game_time_info: GameTimeInfo,

    pub settings: RenderGameSettings,
}

type RenderPlayerHelper = (i32, i32, u32, u32, RenderGameForPlayer);

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
    chat: ChatRender,
    actionfeed: ActionfeedRender,
    scoreboard: ScoreboardRender,
    hud: RenderHud,
    particles: ParticleManager,
    emote_wheel: EmoteWheelRender,
    vote: VoteRender,

    // chat commands
    chat_commands: ChatCommands,

    // map
    map: ClientMapRender,

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
        resource_download_server: Option<Url>,
        config: &ConfigEngine,
        fonts: Arc<UiFontData>,
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
            resource_download_server,
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
            resource_http_download_url.clone(),
            resource_server_download_url.clone(),
            "games-container",
            graphics,
            sound,
            &scene,
            GAME_CONTAINER_PATH.as_ref(),
        );

        let mut creator = UiCreator::default();
        creator.load_font(&fonts);

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
            emote_wheel,
            vote,

            // chat commands
            chat_commands: Default::default(),

            map,

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
        player_info: Option<&RenderForPlayer>,
    ) {
        let map = self.map.try_get().unwrap();

        let mut cam = Camera {
            pos: Default::default(),
            zoom: 1.0,
        };
        let mut cur_anim_time = Duration::ZERO;
        if let Some(local_render_info) = player_info {
            if let Some(character) = render_info
                .character_infos
                .get(&local_render_info.player_id)
                .and_then(|c| c.stage_id.and_then(|id| render_info.stages.get(&id)))
                .and_then(|s| s.world.characters.get(&local_render_info.player_id))
            {
                cam.pos = character.lerped_pos;
                cur_anim_time = RenderMap::calc_anim_time(
                    render_info.game_time_info.ticks_per_second,
                    character.animation_ticks_passed,
                    &render_info.game_time_info.intra_tick_time,
                );
            }
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
        for (_, stage) in render_info.stages.iter() {
            self.render.render(&mut GameObjectsRenderPipe {
                particle_manager: &mut self.particles,
                cur_time,
                game_time_info: &render_info.game_time_info,

                projectiles: &stage.world.projectiles,
                flags: &stage.world.ctf_flags,
                pickups: &stage.world.pickups,
                lasers: &stage.world.lasers,

                ctf_container: &mut self.ctf_container,
                game_container: &mut self.game_container,
                ninja_container: &mut self.ninja_container,
                weapon_container: &mut self.weapon_container,

                camera: &cam,
            });
            self.players.render(&mut PlayerRenderPipe {
                cur_time,
                game_time_info: &render_info.game_time_info,
                render_infos: &stage.world.characters,
                character_infos: &render_info.character_infos,

                particle_manager: &mut self.particles,

                skins: &mut self.skin_container,
                ninjas: &mut self.ninja_container,
                hooks: &mut self.hook_container,
                weapons: &mut self.weapon_container,
                emoticons: &mut self.emoticons_container,

                collision: &render_map.data.collision,
                camera: &cam,

                nameplates: render_info.settings.nameplates,
                own_nameplate: render_info.settings.nameplate_own,
                own_character: player_info.map(|p| &p.player_id),
            });
        }
        let mut render_pipe = RenderPipeline::new(
            &render_map.data.buffered_map.map_visual,
            &render_map.data.buffered_map,
            config_map,
            cur_time,
            &cur_anim_time,
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
        if let Some(local_render_info) = player_info {
            if let Some(player) = render_info
                .character_infos
                .get(&local_render_info.player_id)
                .and_then(|c| c.stage_id.and_then(|id| render_info.stages.get(&id)))
                .and_then(|s| s.world.characters.get(&local_render_info.player_id))
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
        mut player_info: Option<&mut RenderGameForPlayer>,
    ) -> Vec<PlayerFeedbackEvent> {
        let mut res: Vec<PlayerFeedbackEvent> = Default::default();
        // chat & emote wheel
        if let Some(player_render_info) =
            player_info.as_deref_mut().map(|p| &mut p.render_for_player)
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
                        player_id: &player_render_info.player_id,
                        skin_container: &mut self.skin_container,
                        tee_render: &mut self.players.tee_renderer,
                    })
                    .into_iter()
                    .map(PlayerFeedbackEvent::Chat),
            );

            let character_info = render_info
                .character_infos
                .get(&player_render_info.player_id);

            let mut dummy_wheel_ty_ref = &mut None;
            let mut dummy_state = &mut None;

            let wheel_active = if let Some((emote_wheel_ty, emote_state)) =
                &mut player_render_info.emote_wheel_info
            {
                dummy_wheel_ty_ref = emote_wheel_ty;
                dummy_state = emote_state;
                true
            } else {
                false
            };

            if wheel_active {
                let default_key = self.emoticons_container.default_key.clone();
                let skin_default_key = self.skin_container.default_key.clone();
                self.emote_wheel.render(&mut EmoteWheelRenderPipe {
                    cur_time,
                    input: dummy_state,
                    skin_container: &mut self.skin_container,
                    emoticons_container: &mut self.emoticons_container,
                    tee_render: &mut self.players.tee_renderer,
                    emoticons: character_info
                        .map(|c| c.info.emoticons.borrow())
                        .unwrap_or(&*default_key),
                    skin: character_info
                        .map(|c| c.info.skin.borrow())
                        .unwrap_or(&*skin_default_key),
                    skin_info: &character_info.map(|c| c.skin_info),
                });
            }
        }

        // action feed
        self.actionfeed.render(&mut ActionfeedRenderPipe {
            cur_time,
            skin_container: &mut self.skin_container,
            tee_render: &mut self.players.tee_renderer,
        });

        // hud + scoreboard
        if let Some(local_render_info) = player_info.map(|p| &p.render_for_player) {
            let stage = render_info
                .character_infos
                .get(&local_render_info.player_id)
                .and_then(|c| c.stage_id.and_then(|id| render_info.stages.get(&id)));
            let p = stage.and_then(|s| s.world.characters.get(&local_render_info.player_id));
            self.hud.render(&mut RenderHudPipe {
                hud_container: &mut self.hud_container,
                weapon_container: &mut self.weapon_container,
                local_player_render_info: &local_render_info.local_player_info,
                cur_weapon: p.map(|c| c.cur_weapon).unwrap_or_default(),
                race_timer_counter: &p.map(|p| p.game_ticks_passed).unwrap_or_default(),
                ticks_per_second: &render_info.game_time_info.ticks_per_second,
                cur_time,
                game: stage.map(|s| &s.game),
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
                    skin_container: &mut self.skin_container,
                    tee_render: &mut self.players.tee_renderer,
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
                VoteType::Misc() => todo!(),
            } {
                self.vote.render(&mut VoteRenderPipe {
                    cur_time,
                    skin_container: &mut self.skin_container,
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
    fn continue_map_loading(&mut self, config: &ConfigDebug) -> bool;
    fn set_chat_commands(&mut self, chat_commands: ChatCommands);
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
                        killer: killer.and_then(|killer| {
                            character_infos.get(&killer).map(|char| ActionFeedPlayer {
                                name: char.info.name.to_string(),
                                skin: char.info.skin.clone().into(),
                                skin_info: char.skin_info,
                            })
                        }),
                        assists: Vec::new(),
                        victims: victims
                            .iter()
                            .filter_map(|id| {
                                character_infos.get(id).map(|char| ActionFeedPlayer {
                                    name: char.info.name.to_string(),
                                    skin: char.info.skin.clone().into(),
                                    skin_info: char.skin_info,
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

    fn handle_character_event(
        &mut self,
        cur_time: &Duration,
        input: &RenderGameInput,
        pos: vec2,
        ev: GameCharacterEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| input.character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameCharacterEvent::Sound(sound) => match sound {
                GameCharacterEventSound::WeaponSwitch { new_weapon } => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .by_type(new_weapon)
                        .switch
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::NoAmmo { weapon } => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .by_type(weapon)
                        .noammo
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HammerFire => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .hammer
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::GunFire => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .gun
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::GrenadeFire => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::LaserFire => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::ShotgunFire => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .shotgun
                        .weapon
                        .fire
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::GroundJump => {
                    self.skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .ground_jump
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::AirJump => {
                    self.skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .air_jump
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Spawn => {
                    self.skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .spawn
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Death => {
                    self.skin_container
                        .get_or_default_opt(info.map(|i| &i.skin))
                        .sounds
                        .death
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HookHitPlayer => {
                    self.hook_container
                        .get_or_default_opt(info.map(|i| &i.hook))
                        .hit_player
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HookHitHookable => {
                    self.hook_container
                        .get_or_default_opt(info.map(|i| &i.hook))
                        .hit_hookable
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HookHitUnhookable => {
                    self.hook_container
                        .get_or_default_opt(info.map(|i| &i.hook))
                        .hit_unhookable
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Pain { long } => {
                    let sounds = &self
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
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::Hit { strong } => {
                    let sounds = &self
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
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameCharacterEventSound::HammerHit => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .hammer
                        .hits
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
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
                        .player_death(&pos, ColorRGBA::new(1.0, 1.0, 1.0, 1.0));
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
                            self.ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .spawn
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(input.settings.spartial_sound),
                                )
                                .detatch();
                        }
                        GameBuffNinjaEventSound::Collect => {
                            self.ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .collect
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(input.settings.spartial_sound),
                                )
                                .detatch();
                        }
                        GameBuffNinjaEventSound::Attack => {
                            self.ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .attacks
                                .random_entry(&mut self.rng)
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(input.settings.spartial_sound),
                                )
                                .detatch();
                        }
                        GameBuffNinjaEventSound::Hit => {
                            self.ninja_container
                                .get_or_default_opt(info.map(|i| &i.ninja))
                                .hits
                                .random_entry(&mut self.rng)
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(input.settings.spartial_sound),
                                )
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
                                .get_or_default_opt(info.map(|i| &i.freeze))
                                .attack
                                .play(
                                    SoundPlayProps::new_with_pos(pos)
                                        .with_with_spartial(input.settings.spartial_sound),
                                )
                                .detatch();
                        }
                    },
                    GameDebuffFrozenEvent::Effect(ev) => match ev {},
                },
            },
        }
    }

    fn handle_grenade_event(
        &mut self,
        cur_time: &Duration,
        input: &RenderGameInput,
        pos: vec2,
        ev: GameGrenadeEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| input.character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameGrenadeEvent::Sound(ev) => match ev {
                GameGrenadeEventSound::Spawn => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .spawn
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameGrenadeEventSound::Collect => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .collect
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameGrenadeEventSound::Explosion => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .grenade
                        .explosions
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
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
        input: &RenderGameInput,
        pos: vec2,
        ev: GameLaserEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| input.character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameLaserEvent::Sound(ev) => match ev {
                GameLaserEventSound::Spawn => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .spawn
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameLaserEventSound::Collect => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .collect
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameLaserEventSound::Bounce => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .laser
                        .bounces
                        .random_entry(&mut self.rng)
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
            },
            GameLaserEvent::Effect(ev) => match ev {},
        }
    }

    fn handle_shotgun_event(
        &mut self,
        input: &RenderGameInput,
        pos: vec2,
        ev: GameShotgunEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| input.character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameShotgunEvent::Sound(ev) => match ev {
                GameShotgunEventSound::Spawn => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .shotgun
                        .spawn
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameShotgunEventSound::Collect => {
                    self.weapon_container
                        .get_or_default_opt(info.map(|i| &i.weapon))
                        .shotgun
                        .collect
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
            },
            GameShotgunEvent::Effect(ev) => match ev {},
        }
    }

    fn handle_flag_event(
        &mut self,
        input: &RenderGameInput,
        pos: vec2,
        ev: GameFlagEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| input.character_infos.get(&id).map(|c| &c.info));
        match ev {
            GameFlagEvent::Sound(ev) => match ev {
                GameFlagEventSound::Capture => {
                    self.ctf_container
                        .get_or_default_opt(info.map(|i| &i.ctf))
                        .capture
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameFlagEventSound::Collect(ty) => match ty {
                    // TODO: compare to local character team
                    FlagType::Red => {
                        self.ctf_container
                            .get_or_default_opt(info.map(|i| &i.ctf))
                            .collect_team
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(input.settings.spartial_sound),
                            )
                            .detatch();
                    }
                    FlagType::Blue => {
                        self.ctf_container
                            .get_or_default_opt(info.map(|i| &i.ctf))
                            .collect_opponents
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(input.settings.spartial_sound),
                            )
                            .detatch();
                    }
                },
                GameFlagEventSound::Drop => {
                    self.ctf_container
                        .get_or_default_opt(info.map(|i| &i.ctf))
                        .drop
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
                GameFlagEventSound::Return => {
                    self.ctf_container
                        .get_or_default_opt(info.map(|i| &i.ctf))
                        .return_sound
                        .play(
                            SoundPlayProps::new_with_pos(pos)
                                .with_with_spartial(input.settings.spartial_sound),
                        )
                        .detatch();
                }
            },
            GameFlagEvent::Effect(ev) => match ev {},
        }
    }

    fn handle_pickup_event(
        &mut self,
        input: &RenderGameInput,
        pos: vec2,
        ev: GamePickupEvent,
        id: Option<GameEntityId>,
    ) {
        let info = id.and_then(|id| input.character_infos.get(&id).map(|c| &c.info));
        match ev {
            GamePickupEvent::Heart(ev) => match ev {
                GamePickupHeartEvent::Sound(ev) => match ev {
                    GamePickupHeartEventSound::Spawn => {
                        self.game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .heart
                            .spawn
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(input.settings.spartial_sound),
                            )
                            .detatch();
                    }
                    GamePickupHeartEventSound::Collect => {
                        self.game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .heart
                            .collects
                            .random_entry(&mut self.rng)
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(input.settings.spartial_sound),
                            )
                            .detatch();
                    }
                },
                GamePickupHeartEvent::Effect(ev) => match ev {},
            },
            GamePickupEvent::Armor(ev) => match ev {
                GamePickupArmorEvent::Sound(ev) => match ev {
                    GamePickupArmorEventSound::Spawn => {
                        self.game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .shield
                            .spawn
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(input.settings.spartial_sound),
                            )
                            .detatch();
                    }
                    GamePickupArmorEventSound::Collect => {
                        self.game_container
                            .get_or_default_opt(info.map(|i| &i.game))
                            .shield
                            .collects
                            .random_entry(&mut self.rng)
                            .play(
                                SoundPlayProps::new_with_pos(pos)
                                    .with_with_spartial(input.settings.spartial_sound),
                            )
                            .detatch();
                    }
                },
                GamePickupArmorEvent::Effect(ev) => match ev {},
            },
        }
    }

    fn handle_positioned_event(
        &mut self,
        cur_time: &Duration,
        input: &RenderGameInput,
        GameWorldPositionedEvent { ev, pos }: GameWorldPositionedEvent,
    ) {
        match ev {
            GameWorldEntityEvent::Character { ev, id } => {
                self.handle_character_event(cur_time, input, pos, ev, id);
            }
            GameWorldEntityEvent::Grenade { ev, id } => {
                self.handle_grenade_event(cur_time, input, pos, ev, id);
            }
            GameWorldEntityEvent::Laser { id, ev } => {
                self.handle_laser_event(input, pos, ev, id);
            }
            GameWorldEntityEvent::Shotgun { id, ev } => {
                self.handle_shotgun_event(input, pos, ev, id);
            }
            GameWorldEntityEvent::Flag { id, ev } => {
                self.handle_flag_event(input, pos, ev, id);
            }
            GameWorldEntityEvent::Pickup { id, ev } => {
                self.handle_pickup_event(input, pos, ev, id);
            }
        }
    }

    fn handle_events(&mut self, cur_time: &Duration, input: &mut RenderGameInput) {
        // handle events
        let Some(mut events) = input.events.take() else {
            return;
        };
        for (stage_id, mut world) in events.worlds.drain() {
            for (_, ev) in world.events.drain() {
                match ev {
                    GameWorldEvent::Positioned(ev) => {
                        self.handle_positioned_event(cur_time, input, ev)
                    }
                    GameWorldEvent::Global(ev) => match ev {
                        GameWorldGlobalEvent::System(ev) => self.chat.msgs.push_back(MsgInChat {
                            msg: ServerMsg::System(MsgSystem {
                                msg: Self::convert_system_ev(ev),
                            }),
                            add_time: *cur_time,
                        }),
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
        self.chat
            .msgs
            .extend(game.chat_msgs.drain(..).filter_map(|msg| {
                Self::from_net_msg(&game.character_infos, msg).map(|msg| MsgInChat {
                    msg: ServerMsg::Chat(msg),
                    add_time: *cur_time,
                })
            }));
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
                        Some(&RenderForPlayer {
                            chat_info: None,
                            emote_wheel_info: None,
                            local_player_info: *local_player_info,
                            chat_show_all: false,
                            scoreboard_active: false,
                            player_id: *player_id,
                        }),
                    );
                }
                ObservedPlayer::Vote { player_id } => todo!(),
            }
        }
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
        self.skin_container.update(
            cur_time,
            &Duration::from_secs(5),
            &Duration::from_secs(1),
            input
                .character_infos
                .values()
                .map(|info| info.info.skin.borrow()),
        );

        // keep scene active
        self.world_sound_scene.stay_active();

        let mut res = RenderGameResult::default();
        let map = self.map.try_get().unwrap();
        self.particles.update(cur_time, &map.data.collision);

        self.handle_chat_msgs(cur_time, &mut input);
        self.handle_events(cur_time, &mut input);

        let mut next_sound_listeners = self.world_sound_listeners_pool.new();
        std::mem::swap(&mut *next_sound_listeners, &mut self.world_sound_listeners);
        for player in input.players.iter() {
            let p = &player.render_for_player;
            if let Some(c) = input.character_infos.get(&p.player_id).and_then(|c| {
                c.stage_id
                    .and_then(|id| input.stages.get(&id))
                    .and_then(|s| s.world.characters.get(&p.player_id))
            }) {
                if let Some(listener) = next_sound_listeners.remove(&p.player_id) {
                    self.world_sound_listeners
                        .entry(p.player_id)
                        .or_insert(listener)
                } else {
                    self.world_sound_listeners.entry(p.player_id).or_insert(
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
            let _ = self.render_uis(cur_time, &input, None);
        } else {
            let players_per_row = Self::calc_players_per_row(player_count);
            let window_props = self.canvas_handle.window_props();

            let mut helper = self.helper.new();
            let has_viewport_updates = if player_count == 1 {
                let render_for_player_game = input.players.drain(..).next().unwrap();
                let has_observed_players = !render_for_player_game.observed_players.is_empty();
                helper.push((
                    0,
                    0,
                    window_props.window_width,
                    window_props.window_height,
                    render_for_player_game,
                ));
                has_observed_players
            } else {
                helper.extend(input.players.drain(..).enumerate().map(
                    |(index, render_for_player_game)| {
                        let (x, y, w, h) = Self::player_render_area(
                            index,
                            window_props.window_width,
                            window_props.window_height,
                            players_per_row,
                            player_count,
                        );
                        (x, y, w, h, render_for_player_game)
                    },
                ));
                true
            };

            for (x, y, w, h, render_for_player_game) in helper.iter() {
                if has_viewport_updates {
                    self.canvas_handle.update_window_viewport(*x, *y, *w, *h);
                }
                self.render_ingame(
                    config_map,
                    cur_time,
                    &input,
                    Some(&render_for_player_game.render_for_player),
                );

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
                );
            }
            self.backend_handle.consumble_multi_samples();
            for (x, y, w, h, render_for_player_game) in helper.iter_mut() {
                if has_viewport_updates {
                    self.canvas_handle.update_window_viewport(*x, *y, *w, *h);
                }
                let player_id = render_for_player_game.render_for_player.player_id;
                let res_render = self.render_uis(cur_time, &input, Some(render_for_player_game));
                res.player_events.insert(player_id, res_render);
            }
            if has_viewport_updates {
                self.canvas_handle.reset_window_viewport();
            }
        }
        self.particles.update_rates();

        res
    }

    fn continue_map_loading(&mut self, config: &ConfigDebug) -> bool {
        self.map.continue_loading(config).is_some()
    }

    fn set_chat_commands(&mut self, chat_commands: ChatCommands) {
        self.chat_commands = chat_commands
    }
}
