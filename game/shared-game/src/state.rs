pub mod state {
    use std::num::{NonZeroU16, NonZeroU64};
    use std::rc::Rc;
    use std::sync::Arc;

    use base_log::log::SystemLog;
    use game_interface::client_commands::ClientCommand;
    use game_interface::events::{
        EventClientInfo, EventId, EventIdGenerator, GameBuffEvent, GameBuffNinjaEvent,
        GameBuffNinjaEventSound, GameCharacterEvent, GameEvents, GameFlagEvent, GameGrenadeEvent,
        GameGrenadeEventSound, GameLaserEvent, GameLaserEventSound, GamePickupArmorEvent,
        GamePickupArmorEventSound, GamePickupEvent, GamePickupHeartEvent,
        GamePickupHeartEventSound, GameShotgunEvent, GameShotgunEventSound, GameWorldActionFeed,
        GameWorldActionFeedKillWeapon, GameWorldEntityEvent, GameWorldEvent, GameWorldEvents,
        GameWorldGlobalEvent, GameWorldPositionedEvent, GameWorldSystemMessage, KillFeedFlags,
    };
    use game_interface::pooling::GamePooling;
    use game_interface::types::character_info::NetworkCharacterInfo;
    use game_interface::types::game::GameEntityId;
    use game_interface::types::id_gen::IdGenerator;
    use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
    use game_interface::types::pickup::PickupType;
    use game_interface::types::player_info::PlayerClientInfo;
    use game_interface::types::weapons::WeaponType;
    use hiarc::hi_closure;
    use map::map::Map;
    use math::math::vector::vec2;
    use pool::datatypes::{PoolLinkedHashMap, PoolVec};
    use pool::mt_datatypes::{PoolLinkedHashMap as MtPoolLinkedHashMap, PoolVec as MtPoolVec};
    use pool::pool::Pool;

    use game_interface::interface::{GameStateCreate, GameStateInterface, GameStateStaticInfo};
    use game_interface::types::render::character::{
        CharacterBuff, CharacterBuffInfo, CharacterDebuff, CharacterDebuffInfo, CharacterInfo,
        CharacterRenderInfo, LocalCharacterRenderInfo,
    };
    use game_interface::types::render::flag::FlagRenderInfo;
    use game_interface::types::render::laser::LaserRenderInfo;
    use game_interface::types::render::pickup::PickupRenderInfo;
    use game_interface::types::render::projectiles::ProjectileRenderInfo;
    use game_interface::types::render::scoreboard::{
        ScoreboardCharacterInfo, ScoreboardGameType, ScoreboardPlayerSpectatorInfo,
    };
    use game_interface::types::snapshot::{SnapshotClientInfo, SnapshotLocalPlayers};
    use shared_base::mapdef_06::EEntityTiles;

    use crate::collision::collision::Tunings;
    use crate::entities::character::character::{self, CharacterPlayerTy};
    use crate::entities::character::player::player::{
        NoCharPlayer, NoCharPlayerType, NoCharPlayers, Player, PlayerInfo, Players, UknPlayers,
        UnknownPlayer,
    };
    use crate::entities::entity::entity::EntityInterface;
    use crate::entities::flag::flag::Flag;
    use crate::entities::laser::laser::Laser;
    use crate::entities::pickup::pickup::Pickup;
    use crate::entities::projectile::projectile::{self};
    use crate::events::events::{
        CharacterEvent, FlagEvent, LaserEvent, PickupEvent, ProjectileEvent,
    };
    use crate::game_objects::game_objects::GameObjectDefinitions;
    use crate::simulation_pipe::simulation_pipe::{
        SimulationEventWorldEntity, SimulationEvents, SimulationWorldEvent,
    };
    use crate::snapshot::snapshot::SnapshotManager;
    use crate::stage::stage::Stages;
    use crate::types::types::{GameOptions, GameTeam};
    use crate::weapons::definitions::weapon_def::Weapon;
    use crate::world::world::GameWorld;

    use super::super::{
        collision::collision::Collision, entities::character::character::Character,
        simulation_pipe::simulation_pipe::SimulationPipeStage, spawns::GameSpawns,
        stage::stage::GameStage, world::world::WorldPool,
    };

    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum GameError {
        #[error("Stage ID was not found")]
        InvalidStage,
    }

    pub(crate) const TICKS_PER_SECOND: u64 = 50;

    /// A game state is a collection of game related attributes such as the world,
    /// which handles the entities,
    /// the current tick, the starting tick, if the game is paused,
    /// the stages of the game etc.
    pub struct GameState {
        pub(crate) stages: Stages,
        pub(crate) pred_stages: Stages,

        pub players: Players,
        pub no_char_players: NoCharPlayers,

        pub(crate) id_generator: IdGenerator,
        pub(crate) event_id_generator: EventIdGenerator,

        pub simulation_events: SimulationEvents,

        // only useful for client
        pub unknown_players: UknPlayers,

        // only useful for server
        pub stage_0_id: GameEntityId,

        // physics
        pub(crate) collision: Collision,
        spawns: GameSpawns,
        pub(crate) game_objects_definitions: Rc<GameObjectDefinitions>,
        /// empty definitions for prediction
        pub(crate) pred_game_objects_definitions: Rc<GameObjectDefinitions>,

        // game
        pub(crate) game_options: GameOptions,

        // pooling
        pub(crate) world_pool: WorldPool,
        no_char_player_clone_pool: Pool<Vec<NoCharPlayer>>,
        player_clone_pool: Pool<Vec<(GameEntityId, Player)>>,
        game_pools: GamePooling,

        // snapshot
        pub(crate) snap_shot_manager: SnapshotManager,

        // logging
        pub(crate) log: Arc<SystemLog>,
    }

    impl GameStateCreate for GameState {
        fn new(
            map: Vec<u8>,
            options: game_interface::interface::GameStateCreateOptions,
        ) -> (Self, GameStateStaticInfo)
        where
            Self: Sized,
        {
            let physics_group = Map::read_physics_group(&map).unwrap();

            let log = Arc::new(SystemLog::new());
            let w = physics_group.attr.width.get() as u32;
            let h = physics_group.attr.height.get() as u32;

            let tiles = physics_group.get_game_layer_tiles();
            let tune_tiles = physics_group.get_tune_layer_tiles();
            let collision = Collision::new(
                w,
                h,
                tiles,
                tune_tiles.map(|tune_tiles| {
                    (
                        vec![Tunings::default(), {
                            let mut res = Tunings::default();
                            res.grenade_curvature = -70.0;
                            res
                        }],
                        tune_tiles.as_slice(),
                    )
                }),
            );
            let game_objects = GameObjectDefinitions::new(tiles, w, h);

            let mut spawns: Vec<vec2> = Default::default();
            let mut spawns_red: Vec<vec2> = Default::default();
            let mut spawns_blue: Vec<vec2> = Default::default();
            tiles.iter().enumerate().for_each(|(index, tile)| {
                let x = index % w as usize;
                let y = index / w as usize;
                let pos = vec2::new(x as f32 * 32.0 + 1.0, y as f32 * 32.0 + 1.0);
                if tile.index == EEntityTiles::Spawn as u8 {
                    spawns.push(pos);
                } else if tile.index == EEntityTiles::SpawnRed as u8 {
                    spawns_red.push(pos);
                } else if tile.index == EEntityTiles::SpawnBlue as u8 {
                    spawns_blue.push(pos);
                }
            });
            let id_generator = IdGenerator::new();
            let mut game = Self {
                stages: Default::default(),
                pred_stages: Default::default(),

                players: Players::new(),
                no_char_players: NoCharPlayers::new(),

                simulation_events: SimulationEvents::new(),

                // client
                unknown_players: Default::default(),

                // server
                stage_0_id: id_generator.get_next(), // TODO: few lines later the stage_id gets reassigned, but too lazy to improve it rn

                // physics
                collision,
                spawns: GameSpawns {
                    spawns,
                    spawns_red,
                    spawns_blue,
                },
                game_objects_definitions: Rc::new(game_objects),
                pred_game_objects_definitions: Rc::new(GameObjectDefinitions {
                    pickups: Default::default(),
                }),

                // game
                game_options: GameOptions::new(options.game_type),

                world_pool: WorldPool::new(options.hint_max_characters.unwrap_or(64)),
                no_char_player_clone_pool: Pool::with_capacity(2),
                player_clone_pool: Pool::with_capacity(2),
                game_pools: GamePooling::new(options.hint_max_characters),

                id_generator,
                event_id_generator: Default::default(),

                // snapshot
                snap_shot_manager: SnapshotManager::new(&Default::default()),

                // logging
                log: log.clone(),
            };
            game.stage_0_id = game.add_stage();
            (
                game,
                GameStateStaticInfo {
                    ticks_in_a_second: TICKS_PER_SECOND,
                },
            )
        }
    }

    impl GameState {
        fn add_stage(&mut self) -> GameEntityId {
            let stage_id = self.id_generator.get_next();
            self.stages.insert(
                stage_id.clone(),
                GameStage::new(
                    0,
                    stage_id.clone(),
                    &mut self.world_pool,
                    &self.game_objects_definitions,
                    NonZeroU16::new(self.collision.get_playfield_width() as u16).unwrap(),
                    NonZeroU16::new(self.collision.get_playfield_height() as u16).unwrap(),
                    &self.id_generator,
                    self.game_options,
                    &self.log,
                ),
            );
            stage_id
        }

        pub fn add_char_to_stage<'a>(
            stages: &'a mut Stages,
            spawns: &GameSpawns,
            team: Option<GameTeam>,
            stage_id: &GameEntityId,
            character_id: &GameEntityId,
            player_info: PlayerInfo,
            player_input: CharacterInput,
            game_options: &GameOptions,
            players: Players,
            no_char_players: NoCharPlayers,
        ) -> &'a mut Character {
            Self::add_char_to_stage_checked(
                stages,
                spawns,
                team,
                stage_id,
                character_id,
                player_info,
                player_input,
                game_options,
                players,
                no_char_players,
            )
            .unwrap()
        }

        pub(crate) fn add_char_to_stage_checked<'a>(
            stages: &'a mut Stages,
            spawns: &GameSpawns,
            team: Option<GameTeam>,
            stage_id: &GameEntityId,
            character_id: &GameEntityId,
            player_info: PlayerInfo,
            player_input: CharacterInput,
            game_options: &GameOptions,
            players: Players,
            no_char_players: NoCharPlayers,
        ) -> anyhow::Result<&'a mut Character> {
            let stage = stages.get_mut(&stage_id).ok_or(GameError::InvalidStage)?;

            let pos = stage.world.get_spawn_pos(spawns, team);

            let char = stage.world.add_character(
                *character_id,
                stage_id,
                player_info,
                player_input,
                game_options,
                CharacterPlayerTy::Player {
                    players,
                    no_char_players,
                },
                pos,
            );
            Ok(char)
        }

        pub(crate) fn insert_new_stage(
            stages: &mut Stages,
            stage_id: GameEntityId,
            stage_index: usize,
            world_pool: &WorldPool,
            game_object_definitions: &Rc<GameObjectDefinitions>,
            width: NonZeroU16,
            height: NonZeroU16,
            id_gen: &IdGenerator,
            game_options: GameOptions,
            log: &Arc<SystemLog>,
        ) {
            stages.insert(
                stage_id.clone(),
                GameStage::new(
                    stage_index,
                    stage_id.clone(),
                    world_pool,
                    game_object_definitions,
                    width,
                    height,
                    id_gen,
                    game_options,
                    log,
                ),
            );
        }

        fn tick_impl(&mut self, is_prediction: bool) {
            for stage in if !is_prediction {
                &mut self.stages
            } else {
                &mut self.pred_stages
            }
            .values_mut()
            {
                let stage_id = stage.game_element_id.clone();
                let mut sim_pipe = SimulationPipeStage::new(
                    is_prediction,
                    &self.collision,
                    &stage_id,
                    &mut self.id_generator,
                    &mut self.world_pool,
                );

                if !is_prediction {
                    self.simulation_events
                        .insert_world_evs(stage_id, stage.tick(&mut sim_pipe));
                } else {
                    // ignore prediction events
                    let _ = stage.tick(&mut sim_pipe);
                }
            }
        }

        fn on_character_spawn(world: &mut GameWorld, character_id: &GameEntityId) {
            let character = world.characters.get_mut(character_id).unwrap();
            let core = &mut character.get_core_mut();

            core.active_weapon = WeaponType::Gun;

            let gun = Weapon {
                cur_ammo: Some(10),
                next_ammo_regeneration_tick: 0,
            };

            let hammer = Weapon {
                cur_ammo: None,
                next_ammo_regeneration_tick: 0,
            };

            let reusable_core = &mut character.get_reusable_core_mut();
            reusable_core.weapons.insert(WeaponType::Hammer, hammer);
            reusable_core.weapons.insert(WeaponType::Gun, gun);
            reusable_core.weapons.insert(WeaponType::Shotgun, gun);
            reusable_core.weapons.insert(WeaponType::Grenade, gun);
            reusable_core.weapons.insert(WeaponType::Laser, gun);
        }

        pub fn player_tick(&mut self) {
            let mut characters_to_spawn = self.no_char_player_clone_pool.new();
            let characters_to_spawn = &mut characters_to_spawn;
            self.no_char_players.retain_with_order(hi_closure!(
                [
                    characters_to_spawn: &mut PoolVec<NoCharPlayer>
                ],
                |_: &GameEntityId, no_char_player: &mut NoCharPlayer| -> bool {
                    if let NoCharPlayerType::Dead {respawn_in_ticks} = &mut no_char_player.no_char_type {
                        // try to respawn
                        if respawn_in_ticks.tick().unwrap_or_default() {
                            characters_to_spawn.push(no_char_player.clone());
                            false
                        }
                        else {
                            true
                        }
                    } else {
                        true
                    }
                }
            ));

            for no_char_player in characters_to_spawn.drain(..) {
                let last_stage_id = no_char_player.last_stage_id.clone();
                let player_id = no_char_player.id.clone();
                let (char_id, stage_id) = match Self::add_char_to_stage_checked(
                    &mut self.stages,
                    &self.spawns,
                    None,
                    &last_stage_id.unwrap_or(self.stage_0_id),
                    &player_id,
                    no_char_player.player_info.clone(),
                    no_char_player.player_input.clone(),
                    &self.game_options,
                    self.players.clone(),
                    self.no_char_players.clone(),
                ) {
                    Err(_) => (
                        GameState::add_char_to_stage(
                            &mut self.stages,
                            &self.spawns,
                            None,
                            &self.stage_0_id,
                            &player_id,
                            no_char_player.player_info.clone(),
                            no_char_player.player_input.clone(),
                            &self.game_options,
                            self.players.clone(),
                            self.no_char_players.clone(),
                        )
                        .base
                        .game_element_id,
                        self.stage_0_id.clone(),
                    ),
                    Ok(char) => (
                        char.base.game_element_id,
                        last_stage_id.unwrap_or(self.stage_0_id),
                    ),
                };

                GameState::on_character_spawn(
                    &mut self.stages.get_mut(&stage_id).unwrap().world,
                    &char_id,
                );
            }
        }

        fn get_player_input(&self, id: &GameEntityId, player: &Player) -> &CharacterInput {
            &self
                .stages
                .get(&player.stage_id())
                .unwrap()
                .world
                .characters
                .get(id)
                .unwrap()
                .core
                .input
        }

        fn set_player_inp_impl(
            &mut self,
            player_id: &GameEntityId,
            inp: &CharacterInput,
            diff: CharacterInputConsumableDiff,
            is_prediction: bool,
        ) {
            if let Some(player) = self.players.player(player_id) {
                let stages = if !is_prediction {
                    &mut self.stages
                } else {
                    &mut self.pred_stages
                };
                let character = stages
                    .get_mut(&player.stage_id())
                    .unwrap()
                    .world
                    .characters
                    .get_mut(player_id)
                    .unwrap();
                character.core.input = inp.clone();
                stages
                    .get_mut(&player.stage_id())
                    .unwrap()
                    .world
                    .handle_character_input_change(&self.collision, player_id, diff);
            }
        }
    }

    impl GameStateInterface for GameState {
        fn all_projectiles(&self, ratio: f64) -> PoolVec<ProjectileRenderInfo> {
            let mut res = self.game_pools.projectile_render_info_pool.new();
            self.stages.iter().for_each(|(stage_id, stage)| {
                let Some(pred_stage) = self.pred_stages.get(stage_id) else {
                    return;
                };
                res.extend(stage.world.projectiles.iter().filter_map(|(id, proj)| {
                    let Some(pred_proj) = pred_stage.world.projectiles.get(id) else {
                        return None;
                    };
                    Some(ProjectileRenderInfo {
                        ty: proj.projectile.get_core().ty,
                        pos: projectile::lerped_pos(&proj.projectile, &pred_proj.projectile, ratio)
                            / 32.0,
                        vel: projectile::estimated_fly_direction(
                            &proj.projectile,
                            &pred_proj.projectile,
                            ratio,
                        ) / 32.0,
                    })
                }))
            });
            res
        }

        fn all_ctf_flags(&self, ratio: f64) -> PoolVec<FlagRenderInfo> {
            let mut res = self.game_pools.flag_render_info_pool.new();
            self.stages.iter().for_each(|(stage_id, stage)| {
                let Some(pred_stage) = self.pred_stages.get(stage_id) else {
                    return;
                };
                res.extend(stage.world.flags.iter().filter_map(|(id, flag)| {
                    let Some(pred_flag) = pred_stage.world.flags.get(id) else {
                        return None;
                    };
                    Some(FlagRenderInfo {
                        pos: Flag::lerped_pos(&flag, &pred_flag, ratio) / 32.0,
                        ty: flag.core.ty,
                    })
                }))
            });
            res
        }

        fn all_lasers(&self, ratio: f64) -> PoolVec<LaserRenderInfo> {
            let mut res = self.game_pools.laser_render_info_pool.new();
            self.stages.iter().for_each(|(stage_id, stage)| {
                let Some(pred_stage) = self.pred_stages.get(stage_id) else {
                    return;
                };
                res.extend(stage.world.lasers.iter().filter_map(|(id, laser)| {
                    let Some(pred_laser) = pred_stage.world.lasers.get(id) else {
                        return None;
                    };
                    if pred_laser.laser.core.next_eval_in.is_none() {
                        return None;
                    }
                    Some(LaserRenderInfo {
                        ty: laser.laser.get_core().ty,
                        pos: Laser::lerped_pos(&laser.laser, &pred_laser.laser, ratio) / 32.0,
                        from: Laser::lerped_from(&laser.laser, &pred_laser.laser, ratio) / 32.0,
                        eval_tick_ratio: laser.laser.eval_tick_ratio(),
                    })
                }))
            });
            res
        }

        fn all_pickups(&self, ratio: f64) -> PoolVec<PickupRenderInfo> {
            let mut res = self.game_pools.pickup_render_info_pool.new();
            self.stages.iter().for_each(|(stage_id, stage)| {
                let Some(pred_stage) = self.pred_stages.get(stage_id) else {
                    return;
                };
                res.extend(stage.world.pickups.iter().filter_map(|(id, pickup)| {
                    let Some(pred_pickup) = pred_stage.world.pickups.get(id) else {
                        return None;
                    };
                    Some(PickupRenderInfo {
                        ty: pickup.core.ty,
                        pos: Pickup::lerped_pos(pickup, pred_pickup, ratio) / 32.0,
                    })
                }))
            });
            res
        }

        fn collect_characters_render_info(
            &self,
            intra_tick_ratio: f64,
        ) -> PoolLinkedHashMap<GameEntityId, CharacterRenderInfo> {
            let mut render_infos = self.game_pools.character_render_info_pool.new();
            let mut players = self.player_clone_pool.new();
            self.players.pooled_clone_into(&mut players);
            players
                .iter()
                .filter(|(id, p)| {
                    if let Some(stage) = self.stages.get(&p.stage_id()) {
                        if stage.world.characters.contains_key(id) {
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
                .for_each(|(id, p)| {
                    let stage = self.stages.get(&p.stage_id()).unwrap();
                    let player_char = &stage.world.characters[id];
                    let pred_stage = self.pred_stages.get(&p.stage_id()).unwrap_or(stage);
                    let pred_player_char =
                        pred_stage.world.characters.get(id).unwrap_or(player_char);
                    render_infos.insert(
                        *id,
                        CharacterRenderInfo {
                            lerped_pos: character::lerp_core_pos(
                                player_char,
                                pred_player_char,
                                intra_tick_ratio,
                            ) / 32.0,
                            lerped_vel: character::lerp_core_vel(
                                player_char,
                                pred_player_char,
                                intra_tick_ratio,
                            ) / 32.0,
                            lerped_hook_pos: character::lerp_core_hook_pos(
                                player_char,
                                pred_player_char,
                                intra_tick_ratio,
                            )
                            .map(|pos| pos / 32.0),
                            has_air_jump: player_char.core.core.jumped <= 1,
                            cursor_pos: self.get_player_input(id, p).cursor.to_vec2(),
                            move_dir: *self.get_player_input(id, p).state.dir,
                            cur_weapon: player_char.get_core().active_weapon,
                            recoil_ticks_passed: player_char
                                .get_core()
                                .attack_recoil
                                .action_ticks(),
                            right_eye: player_char.core.eye,
                            left_eye: player_char.core.eye,
                            buffs: {
                                let mut buffs = self.game_pools.character_buffs.new();
                                buffs.extend(player_char.reusable_core.buffs.iter().map(
                                    |(buff, _)| match buff {
                                        CharacterBuff::Ninja => (
                                            CharacterBuff::Ninja,
                                            CharacterBuffInfo {
                                                remaining_time: None,
                                            },
                                        ),
                                        CharacterBuff::Ghost => (
                                            CharacterBuff::Ghost,
                                            CharacterBuffInfo {
                                                remaining_time: None,
                                            },
                                        ),
                                    },
                                ));
                                buffs
                            },
                            debuffs: {
                                let mut debuffs = self.game_pools.character_debuffs.new();
                                debuffs.extend(player_char.reusable_core.debuffs.iter().map(
                                    |(debuff, _)| match debuff {
                                        CharacterDebuff::Freeze => (
                                            CharacterDebuff::Freeze,
                                            CharacterDebuffInfo {
                                                remaining_time: None,
                                            },
                                        ),
                                    },
                                ));
                                debuffs
                            },

                            animation_ticks_passed: player_char.core.animation_ticks_passed,
                            game_ticks_passed: player_char.core.game_ticks_passed,
                            game_round_ticks: None,
                        },
                    );
                });
            render_infos
        }

        fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {
            let mut character_infos = self.game_pools.character_info_pool.new();

            let mut players = self.no_char_player_clone_pool.new();
            self.no_char_players.pooled_clone_into(&mut players);
            let no_char_players = players
                .iter()
                .map(|player| (&player.id, &player.player_info));
            // of all chars (even server-side ones)
            // + all non char players
            self.stages
                .values()
                .flat_map(|stage| {
                    stage
                        .world
                        .characters
                        .iter()
                        .map(|(id, character)| (id, &character.reusable_core.player_info))
                })
                .chain(no_char_players)
                .for_each(|(id, info)| {
                    let mut skin = self.game_pools.resource_key_pool.new();
                    skin.clone_from_network(&info.player_info.skin);

                    character_infos.insert(
                        *id,
                        CharacterInfo {
                            name: self
                                .game_pools
                                .string_pool
                                .new_str(info.player_info.name.as_str()),
                            clan: self
                                .game_pools
                                .string_pool
                                .new_str(info.player_info.clan.as_str()),
                            skin,
                            country: self.game_pools.string_pool.new_str("TODO:"),
                        },
                    );
                });

            character_infos
        }

        fn collect_scoreboard_info(&self) -> ScoreboardGameType {
            let mut ingame_scoreboard_infos = self.game_pools.character_scoreboard_pool.new();
            let mut spectator_scoreboard_infos =
                self.game_pools.player_spectator_scoreboard_pool.new();
            let mut players = self.player_clone_pool.new();
            self.players.pooled_clone_into(&mut players);
            players.iter().for_each(|(id, p)| {
                let player_char = self
                    .stages
                    .get(&p.stage_id())
                    .unwrap()
                    .world
                    .characters
                    .get(id)
                    .unwrap();
                ingame_scoreboard_infos.push(ScoreboardCharacterInfo {
                    id: *id,

                    score: player_char.core.score,
                    ping: 0,
                });
            });
            let mut no_char_players = self.no_char_player_clone_pool.new();
            self.no_char_players.pooled_clone_into(&mut no_char_players);
            no_char_players.iter().for_each(|p| {
                spectator_scoreboard_infos.push(ScoreboardPlayerSpectatorInfo {
                    id: p.id,

                    score: 0,
                    ping: 0,
                });
            });
            self.unknown_players.values().for_each(|p| {
                spectator_scoreboard_infos.push(ScoreboardPlayerSpectatorInfo {
                    id: p.id,

                    score: 0,
                    ping: 0,
                });
            });

            ScoreboardGameType::SoloPlay {
                characters: ingame_scoreboard_infos,
                spectator_players: spectator_scoreboard_infos,
            }
        }

        fn collect_character_local_render_info(
            &self,
            player_id: &GameEntityId,
        ) -> LocalCharacterRenderInfo {
            if let Some(p) = self.players.player(player_id) {
                let player_char = self
                    .stages
                    .get(&p.stage_id())
                    .unwrap()
                    .world
                    .characters
                    .get(player_id)
                    .unwrap();

                LocalCharacterRenderInfo {
                    health: player_char.core.health,
                    armor: player_char.core.armor,
                    ammo_of_weapon: player_char.reusable_core.weapons
                        [&player_char.core.active_weapon]
                        .cur_ammo,
                }
            } else {
                LocalCharacterRenderInfo {
                    health: 0,
                    armor: 0,
                    ammo_of_weapon: None,
                }
            }
        }

        fn get_client_camera_join_pos(&self) -> vec2 {
            // TODO:
            vec2::default()
        }

        fn player_join(&mut self, player_info: &PlayerClientInfo) -> GameEntityId {
            let player_id = self.id_generator.get_next();
            let stage_0_id = self.stage_0_id.clone();

            self.stages
                .get(&stage_0_id)
                .unwrap()
                .simulation_events
                .push(SimulationWorldEvent::Global(GameWorldGlobalEvent::System(
                    GameWorldSystemMessage::PlayerJoined {
                        id: player_id,
                        name: self
                            .game_pools
                            .mt_string_pool
                            .new_str(&player_info.info.name),
                    },
                )));

            // spawn and send character info
            let char_id = Self::add_char_to_stage(
                &mut self.stages,
                &self.spawns,
                None,
                &stage_0_id,
                &player_id,
                PlayerInfo {
                    player_info: player_info.info.clone(),
                    version: 1,
                    is_dummy: player_info.is_dummy,
                    unique_identifier: player_info.unique_identifier,
                },
                Default::default(),
                &self.game_options,
                self.players.clone(),
                self.no_char_players.clone(),
            )
            .base
            .game_element_id;
            Self::on_character_spawn(
                &mut self.stages.get_mut(&self.stage_0_id).unwrap().world,
                &char_id,
            );

            player_id
        }

        fn player_drop(&mut self, player_id: &GameEntityId) {
            let name = if let Some(server_player) = self.players.player(player_id) {
                let stage = self.stages.get_mut(&server_player.stage_id()).unwrap();

                let character = stage.world.characters.get_mut(player_id).unwrap();

                let name = self
                    .game_pools
                    .mt_string_pool
                    .new_str(&character.reusable_core.player_info.player_info.name);

                character.despawn_completely_silent();
                stage.world.characters.remove(player_id);

                Some((name, server_player.stage_id()))
            } else if let Some(no_char_player) = self.no_char_players.remove(player_id) {
                let name = self
                    .game_pools
                    .mt_string_pool
                    .new_str(&no_char_player.player_info.player_info.name);
                Some((name, self.stage_0_id))
            } else {
                None
            };

            if let Some((name, stage_id)) = name {
                let stage = self.stages.get(&stage_id).unwrap();
                stage.simulation_events.push(SimulationWorldEvent::Global(
                    GameWorldGlobalEvent::System(GameWorldSystemMessage::PlayerLeft {
                        id: *player_id,
                        name: name,
                    }),
                ));
            }
        }

        fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand) {
            match cmd {
                ClientCommand::Kill => {
                    if let Some(server_player) = self.players.player(player_id) {
                        self.stages
                            .get_mut(&server_player.stage_id())
                            .unwrap()
                            .world
                            .characters
                            .get_mut(player_id)
                            .unwrap()
                            .despawn_to_respawn();
                        self.stages
                            .get_mut(&server_player.stage_id())
                            .unwrap()
                            .world
                            .characters
                            .remove(player_id);
                    }
                }
                ClientCommand::Chat(_) => todo!(),
                ClientCommand::Rcon(_) => todo!(),
            }
        }

        fn try_overwrite_player_character_info(
            &mut self,
            id: &GameEntityId,
            info: &NetworkCharacterInfo,
            version: NonZeroU64,
        ) {
            if let Some(player) = self.players.player(id) {
                let player_info = &mut self
                    .stages
                    .get_mut(&player.stage_id())
                    .unwrap()
                    .world
                    .characters
                    .get_mut(id)
                    .unwrap()
                    .reusable_core
                    .player_info;
                if player_info.version < version.get() {
                    player_info.player_info = info.clone();
                    player_info.version = version.get();
                }
            } else {
                if !self.no_char_players.handle_mut(
                    id,
                    hi_closure!(
                    [version: NonZeroU64, info: &NetworkCharacterInfo],
                    |no_char_player: &mut NoCharPlayer| -> () {
                        if no_char_player.player_info.version < version.get() {
                            no_char_player.player_info.player_info = info.clone();
                            no_char_player.player_info.version = version.get();
                        }
                    }),
                ) {
                    // add as unknown player, the server has to provide a snapshot to make the player useful
                    // this is useful to allow out of order packet arriving
                    if let Some(unkwn_player) = self.unknown_players.get_mut(id) {
                        if unkwn_player.player_info.version < version.get() {
                            unkwn_player.player_info.player_info = info.clone();
                            unkwn_player.player_info.version = version.get();
                        }
                    } else {
                        self.unknown_players.insert(
                            id.clone(),
                            UnknownPlayer::new(
                                PlayerInfo {
                                    player_info: info.clone(),
                                    version: version.get(),
                                    ..Default::default()
                                },
                                id,
                            ),
                        );
                    }
                }
            }
        }

        fn set_player_input(
            &mut self,
            player_id: &GameEntityId,
            inp: &CharacterInput,
            diff: CharacterInputConsumableDiff,
        ) {
            self.set_player_inp_impl(player_id, inp, diff, false)
        }

        fn tick(&mut self) {
            self.tick_impl(false);

            self.player_tick();
        }

        fn pred_tick(
            &mut self,
            mut inps: PoolLinkedHashMap<
                GameEntityId,
                (CharacterInput, CharacterInputConsumableDiff),
            >,
        ) {
            let mut stages = self.snap_shot_manager.snapshot_pool.stages_pool.new();
            self.snap_shot_manager.build_stages(&mut stages, self);
            SnapshotManager::convert_to_game_stages(
                &stages,
                &mut self.pred_stages,
                &mut self.world_pool,
                &self.pred_game_objects_definitions,
                &IdGenerator::new(),
                &self.game_options,
                &self.log,
                &Players::new(),
                &NoCharPlayers::new(),
                &mut UknPlayers::new(),
                NonZeroU16::new(self.collision.get_playfield_width() as u16).unwrap(),
                NonZeroU16::new(self.collision.get_playfield_height() as u16).unwrap(),
            );
            for (ref id, (ref inp, diff)) in inps.drain() {
                self.set_player_inp_impl(id, inp, diff, true);
            }
            self.tick_impl(true);
        }

        fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolVec<u8> {
            let snapshot = self.snap_shot_manager.snapshot_for(&self, client);
            let mut res = self.game_pools.snapshot_pool.new();
            let writer: &mut Vec<_> = &mut res;
            bincode::serde::encode_into_std_write(&snapshot, writer, bincode::config::standard())
                .unwrap();
            res
        }

        /// Writes a snapshot into a game state
        /// It uses a mutable reference to reuse vector capacity, heap objects etc.
        #[must_use]
        fn build_from_snapshot(&mut self, snapshot: &MtPoolVec<u8>) -> SnapshotLocalPlayers {
            let (snapshot, _) =
                bincode::serde::decode_from_slice(&snapshot, bincode::config::standard()).unwrap();

            SnapshotManager::build_from_snapshot(snapshot, self)
        }

        fn events_for(&self, client: EventClientInfo) -> GameEvents {
            // handle simulation events
            let mut worlds_events = self.game_pools.worlds_events_pool.new();
            let mut simulation_events = self.simulation_events.take();

            fn fill_pickup_ev(
                event_id_generator: &EventIdGenerator,
                world_events: &mut MtPoolLinkedHashMap<EventId, GameWorldEvent>,
                id: GameEntityId,
                ev: PickupEvent,
            ) {
                match ev {
                    PickupEvent::Despawn { .. } => {
                        // ignore
                    }
                    PickupEvent::Pickup { pos, ty } => match ty {
                        PickupType::PowerupHealth => {
                            world_events.insert(
                                event_id_generator.next_id(),
                                GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                    pos,
                                    ev: GameWorldEntityEvent::Pickup {
                                        id: Some(id),
                                        ev: GamePickupEvent::Heart(GamePickupHeartEvent::Sound(
                                            GamePickupHeartEventSound::Collect,
                                        )),
                                    },
                                }),
                            );
                        }
                        PickupType::PowerupArmor => {
                            world_events.insert(
                                event_id_generator.next_id(),
                                GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                    pos,
                                    ev: GameWorldEntityEvent::Pickup {
                                        id: Some(id),
                                        ev: GamePickupEvent::Armor(GamePickupArmorEvent::Sound(
                                            GamePickupArmorEventSound::Collect,
                                        )),
                                    },
                                }),
                            );
                        }
                        PickupType::PowerupNinja => {
                            world_events.insert(
                                event_id_generator.next_id(),
                                GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                    pos,
                                    ev: GameWorldEntityEvent::Character {
                                        id: None,
                                        ev: GameCharacterEvent::Buff(GameBuffEvent::Ninja(
                                            GameBuffNinjaEvent::Sound(
                                                GameBuffNinjaEventSound::Collect,
                                            ),
                                        )),
                                    },
                                }),
                            );
                        }
                        PickupType::PowerupWeapon(weapon) => match weapon {
                            WeaponType::Hammer | WeaponType::Gun => {
                                // nothing to do
                            }
                            WeaponType::Shotgun => {
                                world_events.insert(
                                    event_id_generator.next_id(),
                                    GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                        pos,
                                        ev: GameWorldEntityEvent::Shotgun {
                                            id: None,
                                            ev: GameShotgunEvent::Sound(
                                                GameShotgunEventSound::Collect,
                                            ),
                                        },
                                    }),
                                );
                            }
                            WeaponType::Grenade => {
                                world_events.insert(
                                    event_id_generator.next_id(),
                                    GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                        pos,
                                        ev: GameWorldEntityEvent::Grenade {
                                            id: None,
                                            ev: GameGrenadeEvent::Sound(
                                                GameGrenadeEventSound::Collect,
                                            ),
                                        },
                                    }),
                                );
                            }
                            WeaponType::Laser => {
                                world_events.insert(
                                    event_id_generator.next_id(),
                                    GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                        pos,
                                        ev: GameWorldEntityEvent::Laser {
                                            id: None,
                                            ev: GameLaserEvent::Sound(GameLaserEventSound::Collect),
                                        },
                                    }),
                                );
                            }
                        },
                    },
                }
            }

            for (world_id, mut simulation_events) in simulation_events.drain() {
                let mut world_events = self.game_pools.world_events_pool.new();
                for simulation_event in simulation_events.drain(..) {
                    match simulation_event {
                        SimulationWorldEvent::Entity(entity) => match entity {
                            SimulationEventWorldEntity::Character { ev, player_id } => match ev {
                                CharacterEvent::Projectile { .. }
                                | CharacterEvent::Laser { .. } => {
                                    // ignored
                                }
                                CharacterEvent::Despawn { killer_id } => {
                                    world_events.insert(
                                        self.event_id_generator.next_id(),
                                        GameWorldEvent::Global(GameWorldGlobalEvent::ActionFeed(
                                            GameWorldActionFeed::Kill {
                                                killer: killer_id,
                                                assists: self.game_pools.entity_id_pool.new(),
                                                victims: {
                                                    let mut victims =
                                                        self.game_pools.entity_id_pool.new();
                                                    victims.push(player_id);
                                                    victims
                                                },
                                                weapon: GameWorldActionFeedKillWeapon::World,
                                                flags: KillFeedFlags::empty(),
                                            },
                                        )),
                                    );
                                }
                                CharacterEvent::Sound { pos, ev } => {
                                    world_events.insert(
                                        self.event_id_generator.next_id(),
                                        GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                            pos,
                                            ev: GameWorldEntityEvent::Character {
                                                id: Some(player_id),
                                                ev: GameCharacterEvent::Sound(ev),
                                            },
                                        }),
                                    );
                                }
                                CharacterEvent::Effect { pos, ev } => {
                                    world_events.insert(
                                        self.event_id_generator.next_id(),
                                        GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                            pos,
                                            ev: GameWorldEntityEvent::Character {
                                                id: Some(player_id),
                                                ev: GameCharacterEvent::Effect(ev),
                                            },
                                        }),
                                    );
                                }
                                CharacterEvent::Buff { pos, ev } => {
                                    world_events.insert(
                                        self.event_id_generator.next_id(),
                                        GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                            pos,
                                            ev: GameWorldEntityEvent::Character {
                                                id: Some(player_id),
                                                ev: GameCharacterEvent::Buff(ev),
                                            },
                                        }),
                                    );
                                }
                                CharacterEvent::Debuff { pos, ev } => {
                                    world_events.insert(
                                        self.event_id_generator.next_id(),
                                        GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                            pos,
                                            ev: GameWorldEntityEvent::Character {
                                                id: Some(player_id),
                                                ev: GameCharacterEvent::Debuff(ev),
                                            },
                                        }),
                                    );
                                }
                            },
                            SimulationEventWorldEntity::Projectile { id: weapon_id, ev } => {
                                match ev {
                                    ProjectileEvent::Despawn { .. } => {
                                        // nothing to do
                                    }
                                    ProjectileEvent::GrenadeSound { pos, ev } => {
                                        world_events.insert(
                                            self.event_id_generator.next_id(),
                                            GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                                pos,
                                                ev: GameWorldEntityEvent::Grenade {
                                                    id: Some(weapon_id),
                                                    ev: GameGrenadeEvent::Sound(ev),
                                                },
                                            }),
                                        );
                                    }
                                    ProjectileEvent::GrenadeEffect { pos, ev } => {
                                        world_events.insert(
                                            self.event_id_generator.next_id(),
                                            GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                                pos,
                                                ev: GameWorldEntityEvent::Grenade {
                                                    id: Some(weapon_id),
                                                    ev: GameGrenadeEvent::Effect(ev),
                                                },
                                            }),
                                        );
                                    }
                                }
                            }
                            SimulationEventWorldEntity::Pickup { id, ev } => {
                                fill_pickup_ev(&self.event_id_generator, &mut world_events, id, ev);
                            }
                            SimulationEventWorldEntity::Flag { ev, id } => {
                                match ev {
                                    FlagEvent::Despawn { .. } => {
                                        // do nothing
                                    }
                                    FlagEvent::Sound { pos, ev } => {
                                        world_events.insert(
                                            self.event_id_generator.next_id(),
                                            GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                                pos,
                                                ev: GameWorldEntityEvent::Flag {
                                                    id: Some(id),
                                                    ev: GameFlagEvent::Sound(ev),
                                                },
                                            }),
                                        );
                                    }
                                    FlagEvent::Effect { ev, .. } => match ev {},
                                }
                            }
                            SimulationEventWorldEntity::Laser { ev, id } => {
                                match ev {
                                    LaserEvent::Despawn { .. } => {
                                        // do nothing
                                    }
                                    LaserEvent::Sound { pos, ev } => {
                                        world_events.insert(
                                            self.event_id_generator.next_id(),
                                            GameWorldEvent::Positioned(GameWorldPositionedEvent {
                                                pos,
                                                ev: GameWorldEntityEvent::Laser {
                                                    id: Some(id),
                                                    ev: GameLaserEvent::Sound(ev),
                                                },
                                            }),
                                        );
                                    }
                                }
                            }
                        },
                        SimulationWorldEvent::Global(ev) => {
                            world_events.insert(
                                self.event_id_generator.next_id(),
                                GameWorldEvent::Global(ev),
                            );
                        }
                    }
                }
                if !world_events.is_empty() {
                    worlds_events.insert(
                        world_id,
                        GameWorldEvents {
                            events: world_events,
                        },
                    );
                }
            }

            GameEvents {
                worlds: worlds_events,
                event_id: self.event_id_generator.peek_next_id(),
            }
        }

        fn clear_events(&mut self) {}

        fn sync_event_id(&self, event_id: EventId) {
            self.event_id_generator.reset_id_for_client(event_id);
        }
    }
}
