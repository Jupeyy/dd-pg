use std::{sync::Arc, time::Duration};

use base::system::System;
use base_io::io::IO;
use base_io_traits::fs_traits::FileSystemWatcherItemInterface;
use cache::Cache;
use math::math::vector::{dvec2, vec2};
use pool::mt_datatypes::PoolVec;
use shared_base::{game_types::TGameElementID, types::GameTickType};
use wasm_runtime::WasmManager;

use shared_base::network::messages::{MsgObjPlayerInfo, MsgObjPlayerInput, MsgSvPlayerInfo};

use shared_game::{
    entities::{
        flag::flag::FlagRenderInfo, laser::laser::LaserRenderInfo,
        pickup::pickup::PickupRenderInfo, projectile::projectile::ProjectileRenderInfo,
    },
    player::player::{PlayerInput, PlayerRenderInfo},
    snapshot::snapshot::{Snapshot, SnapshotClientInfo},
    state::state::{
        GameState, GameStateCreateOptions, GameStateCreatePipe, GameStateInterface,
        GameStateTickInfo,
    },
    types::types::{PlayerChatInfo, ScoreboardGameType},
};

use super::state_wasm::state_wasm::StateWasm;

pub enum GameStateWrapper {
    Direct(GameState),
    Wasm(StateWasm),
}

impl GameStateWrapper {
    pub fn as_ref(&self) -> &dyn GameStateInterface {
        match self {
            GameStateWrapper::Direct(state) => state,
            GameStateWrapper::Wasm(state) => state,
        }
    }

    pub fn as_mut(&mut self) -> &mut dyn GameStateInterface {
        match self {
            GameStateWrapper::Direct(state) => state,
            GameStateWrapper::Wasm(state) => state,
        }
    }
}

pub struct GameStateWasmManager {
    state: GameStateWrapper,
    fs_change_watcher: Box<dyn FileSystemWatcherItemInterface>,

    // cached values (so wasm module does not need to be called)
    cached_monotonic_tick: GameTickType,
    game_tick_speed: GameTickType,

    /// the time when this state was created or reset (when monotonic tick is set to 0)
    _monotonic_tick_start_time: Duration,
}

const MODS_PATH: &str = "mods/state";

impl GameStateWasmManager {
    pub fn new(
        create_pipe: &GameStateCreatePipe,
        options: &GameStateCreateOptions,
        sys: &System,
        io: &IO,
    ) -> Self {
        let cache = Arc::new(Cache::<0>::new(MODS_PATH, &io.fs));
        // check if loading was finished
        let path_str = MODS_PATH.to_string() + "/state.wasm";
        let fs_change_watcher = io.fs.watch_for_change(MODS_PATH, Some("state.wasm")); // TODO: even tho watching individual files makes more sense, it should still make sure it's the same the server watches
        let task = io.io_batcher.spawn(async move {
            cache
                .load(&path_str, |wasm_bytes| {
                    Ok(WasmManager::compile_module(&wasm_bytes[..])?
                        .serialize()?
                        .to_vec())
                })
                .await
        });
        let (state, game_tick_speed) = if let Ok(wasm_module) = task.get_storage() {
            let state = StateWasm::new(create_pipe, options, &wasm_module);
            let game_tick_speed = state.game_tick_speed();
            (GameStateWrapper::Wasm(state), game_tick_speed)
        } else {
            let state = GameState::new(create_pipe, &sys.log, options);
            let game_tick_speed = state.game_tick_speed();
            (GameStateWrapper::Direct(state), game_tick_speed)
        };
        Self {
            state,
            fs_change_watcher,

            cached_monotonic_tick: 0,
            game_tick_speed,
            _monotonic_tick_start_time: create_pipe.cur_time,
        }
    }

    pub fn should_reload(&self) -> bool {
        self.fs_change_watcher.has_file_change()
    }

    pub fn cur_monotonic_tick(&self) -> GameTickType {
        self.cached_monotonic_tick
    }

    pub fn game_tick_speed(&self) -> GameTickType {
        self.game_tick_speed
    }

    pub fn intra_tick_from_start(
        &self,
        intra_tick_time: &Duration,
        cur_tick: &GameTickType,
        start_tick: &GameTickType,
    ) -> f64 {
        shared_base::state_helper::intra_tick_from_start(
            &self.game_tick_speed(),
            intra_tick_time,
            cur_tick,
            start_tick,
        )
    }
}

impl GameStateInterface for GameStateWasmManager {
    fn lerp_core_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {
        self.state.as_ref().lerp_core_pos(player_id, ratio)
    }

    fn lerp_core_vel(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {
        self.state.as_ref().lerp_core_vel(player_id, ratio)
    }

    fn lerp_core_hook_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {
        self.state.as_ref().lerp_core_hook_pos(player_id, ratio)
    }

    fn cursor_vec2(&self, player_id: &TGameElementID) -> dvec2 {
        self.state.as_ref().cursor_vec2(player_id)
    }

    fn input_dir(&self, player_id: &TGameElementID) -> i32 {
        self.state.as_ref().input_dir(player_id)
    }

    fn add_stage(&mut self) -> TGameElementID {
        self.state.as_mut().add_stage()
    }

    fn stage_count(&self) -> usize {
        self.state.as_ref().stage_count()
    }

    fn generate_next_id(&mut self) -> TGameElementID {
        self.state.as_mut().generate_next_id()
    }

    fn all_players_info(&self, pool: &mut Vec<(TGameElementID, MsgObjPlayerInfo)>) {
        self.state.as_ref().all_players_info(pool)
    }

    fn players_inputs(&self, pool: &mut Vec<(TGameElementID, PlayerInput)>) {
        self.state.as_ref().players_inputs(pool)
    }

    fn all_projectiles(&self, ratio: f64, pool: &mut Vec<ProjectileRenderInfo>) {
        self.state.as_ref().all_projectiles(ratio, pool)
    }

    fn all_ctf_flags(&self, ratio: f64, pool: &mut Vec<FlagRenderInfo>) {
        self.state.as_ref().all_ctf_flags(ratio, pool)
    }

    fn all_lasers(&self, ratio: f64, pool: &mut Vec<LaserRenderInfo>) {
        self.state.as_ref().all_lasers(ratio, pool)
    }

    fn all_pickups(&self, ratio: f64, pool: &mut Vec<PickupRenderInfo>) {
        self.state.as_ref().all_pickups(ratio, pool)
    }

    fn player_exists(&self, player_id: &TGameElementID) -> bool {
        self.state.as_ref().player_exists(player_id)
    }

    fn get_player_and_no_char_player_infos(&self, writer: &mut PoolVec<MsgSvPlayerInfo>) {
        self.state
            .as_ref()
            .get_player_and_no_char_player_infos(writer)
    }

    fn collect_players_render_info(&self, intra_tick_ratio: f64) -> Vec<PlayerRenderInfo> {
        self.state
            .as_ref()
            .collect_players_render_info(intra_tick_ratio)
    }

    fn collect_players_scoreboard_info(&self) -> ScoreboardGameType {
        self.state.as_ref().collect_players_scoreboard_info()
    }

    fn collect_player_chat_info(&self, player_id: &TGameElementID) -> PlayerChatInfo {
        self.state.as_ref().collect_player_chat_info(player_id)
    }

    fn get_client_camera_start_pos(&self) -> vec2 {
        self.state.as_ref().get_client_camera_start_pos()
    }

    fn first_player_id(&self) -> Option<TGameElementID> {
        self.state.as_ref().first_player_id()
    }

    fn player_id_after_id(&self, id: &TGameElementID) -> Option<TGameElementID> {
        self.state.as_ref().player_id_after_id(id)
    }

    fn player_join(&mut self, player_info: &MsgObjPlayerInfo) -> TGameElementID {
        self.state.as_mut().player_join(player_info)
    }

    fn try_player_drop(&mut self, player_id: &TGameElementID) {
        self.state.as_mut().try_player_drop(player_id)
    }

    fn try_overwrite_player_info(
        &mut self,
        id: &TGameElementID,
        info: &MsgObjPlayerInfo,
        version: u64,
    ) {
        self.state
            .as_mut()
            .try_overwrite_player_info(id, info, version)
    }

    fn set_player_inp(
        &mut self,
        player_id: &TGameElementID,
        inp: &MsgObjPlayerInput,
        version: u64,
        force: bool,
    ) {
        self.state
            .as_mut()
            .set_player_inp(player_id, inp, version, force)
    }

    fn tick(&mut self) -> GameStateTickInfo {
        let tick_info = self.state.as_mut().tick();
        self.cached_monotonic_tick = tick_info.cur_game_tick;
        tick_info
    }

    fn pred_tick(&mut self) {
        self.state.as_mut().pred_tick()
    }

    fn build_for(&self, client: SnapshotClientInfo) -> Snapshot {
        self.state.as_ref().build_for(client)
    }

    fn convert_to_game_state(&mut self, snapshot: &Snapshot) -> bool {
        self.cached_monotonic_tick = snapshot.monotonic_tick;
        self.state.as_mut().convert_to_game_state(snapshot)
    }
}
