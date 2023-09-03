pub mod state {
    use std::sync::Arc;
    use std::time::Duration;

    use base_log::log::SystemLog;
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::vector::{dvec2, vec2};
    use pool::mt_datatypes::PoolVec;
    use serde::{Deserialize, Serialize};
    use shared_base::game_types::TGameElementID;

    use shared_base::{
        id_gen::IDGenerator,
        mapdef::{EEntityTiles, MapLayerTile},
        network::messages::{MsgObjPlayerInfo, MsgObjPlayerInput, MsgSvPlayerInfo},
        types::GameTickType,
    };

    use crate::entities::entity::entity::EntityInterface;
    use crate::entities::flag::flag::FlagRenderInfo;
    use crate::entities::laser::laser::LaserRenderInfo;
    use crate::entities::pickup::pickup::PickupRenderInfo;
    use crate::entities::projectile::projectile::ProjectileRenderInfo;
    use crate::player::player::PlayerRenderInfo;
    use crate::snapshot::snapshot::{Snapshot, SnapshotClientInfo, SnapshotManager};
    use crate::weapons::definitions::weapon_def::{Weapon, WeaponType};

    use super::super::{
        collision::collision::Collision,
        entities::character::character::Character,
        player::player::{
            NoCharPlayer, NoCharPlayerType, NoCharPlayers, Player, PlayerCharacterInfo,
            PlayerInput, Players, UnknownPlayer,
        },
        simulation_pipe::simulation_pipe::{SimulationEvents, SimulationPipeStage},
        spawns::GameSpawns,
        stage::stage::GameStage,
        world::world::WorldPool,
    };

    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum GameError {
        #[error("Stage ID was not found")]
        InvalidStage,
    }

    pub trait GameStateInterface {
        fn lerp_core_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2;
        fn lerp_core_vel(&self, player_id: &TGameElementID, ratio: f64) -> vec2;
        fn lerp_core_hook_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2;
        fn cursor_vec2(&self, player_id: &TGameElementID) -> dvec2;
        fn input_dir(&self, player_id: &TGameElementID) -> i32;

        /// TODO: remove this
        fn set_cur_monotonic_tick(&mut self, cur_monotonic_tick: GameTickType);

        fn first_player_id(&self) -> Option<TGameElementID>;
        fn player_id_after_id(&self, id: &TGameElementID) -> Option<TGameElementID>;

        // TODO: needed?
        fn add_stage(&mut self) -> TGameElementID;
        fn stage_count(&self) -> usize;

        /// generate a new unique ID that can be used for any game element
        fn generate_next_id(&mut self) -> TGameElementID;

        fn player_join(&mut self, player_info: &MsgObjPlayerInfo) -> TGameElementID;
        fn try_player_drop(&mut self, player_id: &TGameElementID);

        /// player info of all players.. those with characters and those without
        fn all_players_info(&self, pool: &mut Vec<(TGameElementID, MsgObjPlayerInfo)>);
        fn players_inputs(&self, pool: &mut Vec<(TGameElementID, PlayerInput)>);

        // stuff that is rendered
        fn all_projectiles(&self, ratio: f64, pool: &mut Vec<ProjectileRenderInfo>);
        fn all_ctf_flags(&self, ratio: f64, pool: &mut Vec<FlagRenderInfo>);
        fn all_lasers(&self, ratio: f64, pool: &mut Vec<LaserRenderInfo>);
        fn all_pickups(&self, ratio: f64, pool: &mut Vec<PickupRenderInfo>);

        #[must_use]
        fn player_exists(&self, player_id: &TGameElementID) -> bool;

        fn get_player_and_no_char_player_infos(&self, writer: &mut PoolVec<MsgSvPlayerInfo>);

        /// try to override the player info, if the new info is newer
        /// also checks for all types of players (no char, unknown)
        fn try_overwrite_player_info(
            &mut self,
            id: &TGameElementID,
            info: &MsgObjPlayerInfo,
            version: u64,
        );

        fn set_player_inp(
            &mut self,
            player_id: &TGameElementID,
            inp: &MsgObjPlayerInput,
            version: u64,
            force: bool,
        );

        fn collect_players_render_info(
            &self,
            intra_tick_ratio: f64,
            render_infos: &mut Vec<PlayerRenderInfo>,
        );

        /// retrieve a position the client should first see when connecting
        fn get_client_camera_start_pos(&self) -> vec2;

        /// get the current monotonic tick, this tick should never
        /// go backwards or skip a tick
        fn tick(&mut self) -> GameTickType;
        fn pred_tick(&mut self);

        // snapshot related
        /// builds a snapshot out of the current game state
        #[must_use]
        fn build_for(&self, client: SnapshotClientInfo) -> Snapshot;

        /**
         * Writes a snapshot into a game state
         * It uses a mutable reference to reuse vector capacity, heap objects etc.
         */
        #[must_use]
        fn convert_to_game_state(&mut self, snapshot: &Snapshot) -> bool;
    }

    type UknPlayers = LinkedHashMap<TGameElementID, UnknownPlayer>;

    pub type Stages = LinkedHashMap<TGameElementID, GameStage>;

    #[derive(Debug, Default, Serialize, Deserialize, Encode, Decode)]
    pub struct GameStateCreateOptions {
        // the max number of characters is usually also used for
        // the number of players, the number of stages etc.
        hint_max_characters: Option<usize>,
    }

    pub struct GameStateCreatePipe<'a> {
        pub game_layer: &'a MapLayerTile,
        pub cur_time: Duration,
    }

    pub(crate) const TICKS_PER_SECOND: u64 = 50;
    // set an offset of around 1 day to prevent problems with low tick counts when
    // taking differences with the `cur_monotonic_tick`
    pub const MONOTONIC_TICK_OFFSET: GameTickType = 50 * 60 * 60 * 24;

    /**
     * A game state is a collection of game related attributes such as the world,
     * which handles the entities,
     * the current tick, the starting tick, if the game is paused,
     * the stages of the game etc.
     */
    pub struct GameState {
        pub(crate) stages: Stages,

        /// the monotonic tick is an ever increasing tick
        pub(crate) cur_monotonic_tick: u64,

        pub players: Players,
        pub no_char_players: NoCharPlayers,

        id_generator: IDGenerator,

        pub simulation_events: Vec<SimulationEvents>,

        // only useful for client
        pub unknown_players: UknPlayers,

        // only useful for server
        pub stage_0_id: TGameElementID,

        // physics
        collision: Collision,
        spawns: GameSpawns,

        // pooling
        pub(crate) world_pool: WorldPool,

        // snapshot
        pub(crate) snap_shot_manager: SnapshotManager,

        // logging
        log: Arc<SystemLog>,
    }

    impl GameState {
        pub fn new(
            create_pipe: &GameStateCreatePipe,
            log: &Arc<SystemLog>,
            options: &GameStateCreateOptions,
        ) -> Self {
            let collision: Collision;
            let game_layer = create_pipe.game_layer;
            let w = game_layer.0.width as u32;
            let h = game_layer.0.height as u32;

            let tiles = game_layer.2.as_slice();
            collision = Collision::new(w, h, tiles);

            let mut spawns: Vec<vec2> = Default::default();
            let mut spawns_red: Vec<vec2> = Default::default();
            let mut spawns_blue: Vec<vec2> = Default::default();
            game_layer.2.iter().enumerate().for_each(|(index, tile)| {
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
            let mut game = Self {
                cur_monotonic_tick: MONOTONIC_TICK_OFFSET,

                stages: Default::default(),

                players: Default::default(),
                no_char_players: Default::default(),

                id_generator: IDGenerator::new(),

                simulation_events: Default::default(),

                // client
                unknown_players: Default::default(),

                // server
                stage_0_id: Default::default(),

                // physics
                collision,
                spawns: GameSpawns {
                    spawns,
                    spawns_red,
                    spawns_blue,
                },

                world_pool: WorldPool::new(options.hint_max_characters.unwrap_or(64)),

                // snapshot
                snap_shot_manager: SnapshotManager::new(&Default::default()),

                // logging
                log: log.clone(),
            };
            game.stage_0_id = game.add_stage();
            game
        }

        pub fn game_tick_speed(&self) -> GameTickType {
            TICKS_PER_SECOND
        }

        pub fn get_stage_by_id_mut(&mut self, id: &TGameElementID) -> &mut GameStage {
            self.stages.get_mut(id).unwrap()
        }

        pub fn get_stage_by_id(&self, id: &TGameElementID) -> &GameStage {
            self.stages.get(id).unwrap()
        }

        pub fn get_stage_by_id_checked_mut(
            &mut self,
            id: &TGameElementID,
        ) -> Option<&mut GameStage> {
            self.stages.get_mut(id)
        }

        pub fn get_stage_by_checked_id(&self, id: &TGameElementID) -> Option<&GameStage> {
            self.stages.get(id)
        }

        pub fn get_stages(&self) -> &Stages {
            &self.stages
        }

        pub fn get_stages_mut(&mut self) -> &mut Stages {
            &mut self.stages
        }

        pub fn contains_character(
            &mut self,
            stage_id: &TGameElementID,
            character_id: &TGameElementID,
        ) -> bool {
            self.stages
                .get(stage_id)
                .unwrap()
                .world
                .get_characters()
                .contains_key(character_id)
        }

        pub fn add_char_to_stage<'a>(
            stages: &'a mut Stages,
            id_generator: &mut IDGenerator,
            stage_id: &TGameElementID,
            characters_pool: &mut WorldPool,
        ) -> &'a mut Character {
            stages
                .get_mut(&stage_id)
                .unwrap()
                .world
                .add_character(id_generator, characters_pool)
        }

        pub(crate) fn add_char_to_stage_checked<'a>(
            stages: &'a mut Stages,
            id_generator: &mut IDGenerator,
            stage_id: &TGameElementID,
            characters_pool: &mut WorldPool,
        ) -> anyhow::Result<&'a mut Character> {
            Ok(stages
                .get_mut(&stage_id)
                .ok_or(GameError::InvalidStage)?
                .world
                .add_character(id_generator, characters_pool))
        }

        pub fn contains_projectile(
            &mut self,
            stage_id: &TGameElementID,
            projectile_id: &TGameElementID,
        ) -> bool {
            self.stages
                .get(stage_id)
                .unwrap()
                .world
                .get_projectiles()
                .contains_key(projectile_id)
        }

        pub(crate) fn insert_new_projectile_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            projectile_id: TGameElementID,
            owner_character_id: TGameElementID,
            pos: &vec2,
            direction: &vec2,
            life_span: i32,
            damage: u32,
            force: f32,
            start_tick: GameTickType,
            explosive: bool,
        ) {
            self.stages
                .get_mut(stage_id)
                .unwrap()
                .world
                .insert_new_projectile(
                    projectile_id,
                    owner_character_id,
                    pos,
                    direction,
                    life_span,
                    damage,
                    force,
                    start_tick,
                    explosive,
                    &mut self.world_pool,
                );
        }

        pub fn get_player_by_id_checked_mut(&mut self, id: &TGameElementID) -> Option<&mut Player> {
            self.players.get_mut(id)
        }

        pub fn get_player_by_id_checked(&self, id: &TGameElementID) -> Option<&Player> {
            self.players.get(id)
        }

        pub fn get_no_char_player_by_id_checked_mut(
            &mut self,
            id: &TGameElementID,
        ) -> Option<&mut NoCharPlayer> {
            self.no_char_players.get_mut(id)
        }

        pub fn get_no_char_player_by_id_checked(
            &self,
            id: &TGameElementID,
        ) -> Option<&NoCharPlayer> {
            self.no_char_players.get(id)
        }

        /// this moves the player from players without chars to players
        fn remove_char_from_player(
            players: &mut Players,
            no_char_players: &mut NoCharPlayers,
            player_id: &TGameElementID,
            last_stage_id: &TGameElementID,
            respawns_at_tick: GameTickType,
            no_char_type: NoCharPlayerType,
        ) {
            let player = players.remove(player_id).unwrap();
            no_char_players.insert(
                player_id.clone(),
                NoCharPlayer {
                    player_info: player.player_info,
                    id: player_id.clone(),
                    version: player.version,
                    last_stage_id: last_stage_id.clone(),
                    respawns_at_tick,
                    no_char_type,
                },
            );
        }

        /// this moves the player from players without chars to players
        fn give_player_a_char(
            players: &mut Players,
            no_char_player: &mut NoCharPlayer,
            player_id: &TGameElementID,
            character_info: PlayerCharacterInfo,
            game_start_tick: GameTickType,
            animation_start_tick: GameTickType,
        ) {
            players.insert(
                player_id.clone(),
                Player {
                    player_info: no_char_player.player_info.clone(),
                    input: PlayerInput::default(),
                    id: player_id.clone(),
                    character_info,
                    version: no_char_player.version,
                    game_start_tick,
                    animation_start_tick,
                },
            );
        }

        pub(crate) fn insert_new_stage(&mut self, stage_id: TGameElementID, stage_index: usize) {
            self.stages.insert(
                stage_id.clone(),
                GameStage::new(
                    stage_index,
                    stage_id.clone(),
                    &mut self.world_pool,
                    &self.log,
                ),
            );
        }

        pub(crate) fn insert_new_character_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            character_id: TGameElementID,
        ) {
            self.stages
                .get_mut(stage_id)
                .unwrap()
                .world
                .insert_new_character(character_id, &mut self.world_pool);
        }

        fn get_spawn_pos(spawns: &GameSpawns) -> vec2 {
            spawns.spawns.get(0).copied().unwrap_or(
                spawns
                    .spawns_red
                    .get(0)
                    .copied()
                    .unwrap_or(spawns.spawns_blue.get(0).copied().unwrap_or_default()),
            )
        }

        fn tick_impl(&mut self, is_prediction: bool) {
            for stage in &mut self.stages.values_mut() {
                let stage_id = stage.game_element_id.clone();
                let mut sim_pipe = SimulationPipeStage::new(
                    0,
                    if is_prediction { 1 } else { 0 },
                    &self.players,
                    is_prediction,
                    &self.collision,
                    &stage_id,
                    self.cur_monotonic_tick,
                    &mut self.simulation_events,
                    &mut self.id_generator,
                    &mut self.world_pool,
                );
                stage.tick(&mut sim_pipe);
            }
        }

        fn on_character_spawn(spawns: &GameSpawns, character: &mut Character) {
            let pos = Self::get_spawn_pos(spawns);
            let core = &mut character.get_core_mut();

            core.core.pos.x = pos.x;
            core.core.pos.y = pos.y;

            let new_weapon = WeaponType::Gun;
            core.active_weapon = new_weapon;

            let weapon = Weapon {
                cur_ammo: 10,
                next_ammo_regeneration_tick: 0,
            };

            let reusable_core = &mut character.get_reusable_core_mut();
            reusable_core.weapons.insert(new_weapon, weapon);
        }

        pub fn player_tick(&mut self) {
            let cur_tick = self.cur_monotonic_tick;
            self.no_char_players
                .retain_with_order(|id, no_char_player| {
                    if let NoCharPlayerType::Dead = no_char_player.no_char_type {
                        // try to respawn
                        if cur_tick > no_char_player.respawns_at_tick {
                            let last_stage_id = no_char_player.last_stage_id.clone();
                            let player_id = id.clone();
                            let (char, stage_id) = match Self::add_char_to_stage_checked(
                                &mut self.stages,
                                &mut self.id_generator,
                                &last_stage_id,
                                &mut self.world_pool,
                            ) {
                                Err(_) => (
                                    Self::add_char_to_stage(
                                        &mut self.stages,
                                        &mut self.id_generator,
                                        &self.stage_0_id,
                                        &mut self.world_pool,
                                    ),
                                    self.stage_0_id.clone(),
                                ),
                                Ok(char) => (char, last_stage_id),
                            };
                            Self::on_character_spawn(&self.spawns, char);
                            let char_id = char.base.game_element_id.clone();
                            Self::give_player_a_char(
                                &mut self.players,
                                no_char_player,
                                &player_id,
                                PlayerCharacterInfo {
                                    character_id: char_id,
                                    stage_id,
                                },
                                self.cur_monotonic_tick, // TODO: have smth like a round start tick
                                self.cur_monotonic_tick, // TODO: have smth like a round start tick
                            );
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                });
        }
    }

    impl GameStateInterface for GameState {
        fn lerp_core_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {
            let char_info = &self.players.get(player_id).unwrap().character_info;
            let char = self
                .stages
                .get(&char_info.stage_id)
                .unwrap()
                .world
                .get_character_by_id(&char_info.character_id);
            char.lerp_core_pos(ratio)
        }

        fn lerp_core_vel(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {
            let char_info = &self.players.get(player_id).unwrap().character_info;
            let char = self
                .stages
                .get(&char_info.stage_id)
                .unwrap()
                .world
                .get_character_by_id(&char_info.character_id);
            char.lerp_core_vel(ratio)
        }

        fn lerp_core_hook_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {
            let char_info = &self.players.get(player_id).unwrap().character_info;
            let char = self
                .stages
                .get(&char_info.stage_id)
                .unwrap()
                .world
                .get_character_by_id(&char_info.character_id);
            char.lerp_core_hook_pos(ratio)
        }

        fn cursor_vec2(&self, player_id: &TGameElementID) -> dvec2 {
            let player = self.players.get(player_id).unwrap();
            player.input.inp.cursor.to_vec2()
        }

        fn input_dir(&self, player_id: &TGameElementID) -> i32 {
            let player = self.players.get(player_id).unwrap();
            *player.input.inp.dir
        }

        fn set_cur_monotonic_tick(&mut self, cur_monotonic_tick: GameTickType) {
            self.cur_monotonic_tick = cur_monotonic_tick;
        }

        fn add_stage(&mut self) -> TGameElementID {
            let stage_id = self.id_generator.get_next();
            self.stages.insert(
                stage_id.clone(),
                GameStage::new(0, stage_id.clone(), &mut self.world_pool, &self.log),
            );
            stage_id
        }

        fn stage_count(&self) -> usize {
            self.stages.len()
        }

        fn generate_next_id(&mut self) -> TGameElementID {
            self.id_generator.get_next()
        }

        fn all_players_info(&self, pool: &mut Vec<(TGameElementID, MsgObjPlayerInfo)>) {
            pool.reserve(
                self.players.len() + self.no_char_players.len() + self.unknown_players.len(),
            );
            self.players.iter().for_each(|(id, player)| {
                pool.push((id.clone(), player.player_info.clone()));
            });
            self.no_char_players.iter().for_each(|(id, player)| {
                pool.push((id.clone(), player.player_info.clone()));
            });
            self.unknown_players.iter().for_each(|(id, player)| {
                pool.push((id.clone(), player.player_info.clone()));
            });
        }

        fn players_inputs(&self, pool: &mut Vec<(TGameElementID, PlayerInput)>) {
            pool.extend(
                self.players
                    .iter()
                    .map(|(id, player)| (id.clone(), player.input.clone())),
            );
        }

        fn all_projectiles(&self, ratio: f64, pool: &mut Vec<ProjectileRenderInfo>) {
            self.stages.values().for_each(|stage| {
                pool.append(
                    &mut stage
                        .world
                        .projectiles
                        .values()
                        .map(|proj| ProjectileRenderInfo {
                            ty: proj.projectile.get_core().ty,
                            pos: proj.projectile.lerped_pos(ratio),
                            vel: proj.projectile.estimated_fly_direction(ratio),
                        })
                        .collect::<Vec<ProjectileRenderInfo>>(),
                )
            });
        }

        fn all_ctf_flags(&self, _ratio: f64, _pool: &mut Vec<FlagRenderInfo>) {}

        fn all_lasers(&self, _ratio: f64, _pool: &mut Vec<LaserRenderInfo>) {}

        fn all_pickups(&self, _ratio: f64, _pool: &mut Vec<PickupRenderInfo>) {}

        fn player_exists(&self, player_id: &TGameElementID) -> bool {
            self.players.contains_key(player_id)
        }

        fn get_player_and_no_char_player_infos(&self, writer: &mut PoolVec<MsgSvPlayerInfo>) {
            self.players.values().for_each(|player| {
                writer.push(MsgSvPlayerInfo {
                    id: player.id.clone(),
                    info: player.player_info.clone(),
                    version: player.version,
                });
            });
            self.no_char_players.values().for_each(|player| {
                writer.push(MsgSvPlayerInfo {
                    id: player.id.clone(),
                    info: player.player_info.clone(),
                    version: player.version,
                });
            });
        }

        fn collect_players_render_info(
            &self,
            intra_tick_ratio: f64,
            render_infos: &mut Vec<PlayerRenderInfo>,
        ) {
            self.players
                .values()
                .filter(|p| {
                    if let Some(stage) = self.stages.get(&p.character_info.stage_id) {
                        if stage
                            .world
                            .get_characters()
                            .contains_key(&p.character_info.character_id)
                        {
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
                .for_each(|p| {
                    let player_char = self
                        .stages
                        .get(&p.character_info.stage_id)
                        .unwrap()
                        .world
                        .get_character_by_id(&p.character_info.character_id);
                    render_infos.push(PlayerRenderInfo {
                        lerped_pos: player_char.lerp_core_pos(intra_tick_ratio),
                        lerped_vel: player_char.lerp_core_vel(intra_tick_ratio),
                        lerped_hook_pos: player_char.lerp_core_hook_pos(intra_tick_ratio),
                        hook_state: player_char.get_core().core.hook_state,
                        cursor_pos: p.input.inp.cursor.to_vec2(),
                        move_dir: *p.input.inp.dir,
                        cur_weapon: player_char.get_core().active_weapon,
                        recoil_start_tick: player_char.get_core().recoil_start_tick,
                    });
                });
        }

        fn get_client_camera_start_pos(&self) -> vec2 {
            Self::get_spawn_pos(&self.spawns)
        }

        fn first_player_id(&self) -> Option<TGameElementID> {
            self.players.front().map(|p| p.0.clone())
        }

        fn player_id_after_id(&self, id: &TGameElementID) -> Option<TGameElementID> {
            let mut it = self.players.iter_at_key(&id).unwrap();
            // current id
            it.next();
            // next id
            it.next().map(|(id, _)| id.clone())
        }

        fn player_join(&mut self, player_info: &MsgObjPlayerInfo) -> TGameElementID {
            let player_id = self.generate_next_id();
            let stage_0_id = self.stage_0_id.clone();

            // spawn and send character info
            let char = Self::add_char_to_stage(
                &mut self.stages,
                &mut self.id_generator,
                &stage_0_id,
                &mut self.world_pool,
            );
            Self::on_character_spawn(&self.spawns, char);

            let char_id = char.base.game_element_id.clone();

            self.players.insert(
                player_id.clone(),
                Player::new(
                    player_info.clone(),
                    &player_id,
                    PlayerCharacterInfo {
                        character_id: char_id.clone(),
                        stage_id: self.stage_0_id.clone(),
                    },
                    0,
                    self.cur_monotonic_tick, // TODO: round start tick or smth
                    self.cur_monotonic_tick, // TODO: round start tick or smth
                ),
            );

            self.players.get_mut(&player_id).unwrap().character_info = PlayerCharacterInfo {
                character_id: char.base.game_element_id.clone(),
                stage_id: self.stage_0_id.clone(),
            };

            player_id
        }

        fn try_player_drop(&mut self, player_id: &TGameElementID) {
            if let Some(server_player) = self.players.remove(player_id) {
                let char = &server_player.character_info;
                self.get_stage_by_id_mut(&char.stage_id)
                    .world
                    .rem_character(&char.character_id);
            }
        }

        fn try_overwrite_player_info(
            &mut self,
            id: &TGameElementID,
            info: &MsgObjPlayerInfo,
            version: u64,
        ) {
            if let Some(player) = self.players.get_mut(id) {
                if player.version < version {
                    player.player_info = info.clone();
                    player.version = version;
                }
            } else {
                if let Some(no_char_player) = self.no_char_players.get_mut(id) {
                    if no_char_player.version < version {
                        no_char_player.player_info = info.clone();
                        no_char_player.version = version;
                    }
                } else {
                    // add as unknown player, the server has to provide a snapshot to make the player useful
                    // this is useful to allow out of order packet arriving
                    if let Some(unkwn_player) = self.unknown_players.get_mut(id) {
                        if unkwn_player.version < version {
                            unkwn_player.player_info = info.clone();
                            unkwn_player.version = version;
                        }
                    } else {
                        self.unknown_players
                            .insert(id.clone(), UnknownPlayer::new(info, id, version));
                    }
                }
            }
        }

        fn set_player_inp(
            &mut self,
            player_id: &TGameElementID,
            inp: &MsgObjPlayerInput,
            version: u64,
            force: bool,
        ) {
            if let Some(player) = self.players.get_mut(player_id) {
                if player.input.version < version || force {
                    player.input.inp = *inp;
                    player.input.version = version;
                }
            }
        }

        fn tick(&mut self) -> GameTickType {
            self.tick_impl(false);

            // handle simulation events
            self.simulation_events.drain(..).for_each(|ev| match ev {
                SimulationEvents::PlayerCharacterRemoved { id, remove_info } => {
                    Self::remove_char_from_player(
                        &mut self.players,
                        &mut self.no_char_players,
                        &id,
                        &remove_info.last_stage_id,
                        remove_info.respawns_at_tick.unwrap_or(0),
                        remove_info.no_char_type,
                    );
                }
            });

            self.player_tick();

            self.cur_monotonic_tick += 1;
            self.cur_monotonic_tick
        }

        fn pred_tick(&mut self) {
            self.tick_impl(true);

            // ignore world events
            self.simulation_events.clear();
        }

        fn build_for(&self, client: SnapshotClientInfo) -> Snapshot {
            self.snap_shot_manager.build_for(&self, client)
        }

        /**
         * Writes a snapshot into a game state
         * It uses a mutable reference to reuse vector capacity, heap objects etc.
         */
        #[must_use]
        fn convert_to_game_state(&mut self, snapshot: &Snapshot) -> bool {
            SnapshotManager::convert_to_game_state(snapshot, self)
        }
    }
}
