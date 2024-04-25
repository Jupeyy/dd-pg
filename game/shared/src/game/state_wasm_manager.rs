use std::num::NonZeroU64;
use std::sync::Arc;

use base_io::io::IO;
use base_io_traits::fs_traits::FileSystemWatcherItemInterface;
use cache::Cache;
//use ddnet::Ddnet;
use game_interface::client_commands::ClientCommand;
use game_interface::events::{EventClientInfo, EventId, GameEvents};
use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
use game_interface::types::character_info::NetworkCharacterInfo;
use game_interface::types::game::{GameEntityId, GameTickType};
use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
use game_interface::types::player_info::PlayerClientInfo;
use game_interface::types::render::character::CharacterInfo;
use math::math::vector::vec2;
use pool::datatypes::{PoolLinkedHashMap, PoolVec};
use pool::mt_datatypes::PoolVec as MtPoolVec;
use shared_game::state::state::GameState;
use wasm_runtime::WasmManager;

use game_interface::{
    interface::GameStateInterface,
    types::{
        render::{
            character::{CharacterRenderInfo, LocalCharacterRenderInfo},
            flag::FlagRenderInfo,
            laser::LaserRenderInfo,
            pickup::PickupRenderInfo,
            projectiles::ProjectileRenderInfo,
            scoreboard::ScoreboardGameType,
        },
        snapshot::{SnapshotClientInfo, SnapshotLocalPlayers},
    },
};

use super::state_wasm::state_wasm::StateWasm;

enum GameStateWrapper {
    Native(GameState),
    //Ddnet(Ddnet),
    Wasm(StateWasm),
}

impl GameStateWrapper {
    pub fn as_ref(&self) -> &dyn GameStateInterface {
        match self {
            GameStateWrapper::Native(state) => state,
            //GameStateWrapper::Ddnet(state) => state,
            GameStateWrapper::Wasm(state) => state,
        }
    }

    pub fn as_mut(&mut self) -> &mut dyn GameStateInterface {
        match self {
            GameStateWrapper::Native(state) => state,
            //GameStateWrapper::Ddnet(state) => state,
            GameStateWrapper::Wasm(state) => state,
        }
    }
}

pub struct GameStateWasmManager {
    state: GameStateWrapper,
    fs_change_watcher: Box<dyn FileSystemWatcherItemInterface>,

    info: GameStateStaticInfo,

    pub predicted_game_monotonic_tick: GameTickType,
}

const MODS_PATH: &str = "mods/state";

impl GameStateWasmManager {
    pub fn new(map: Vec<u8>, options: GameStateCreateOptions, io: &IO) -> Self {
        let cache = Arc::new(Cache::<0>::new(MODS_PATH, &io.fs));
        // check if loading was finished
        let wasm_path_str = MODS_PATH.to_string() + "/state.wasm";
        let fs_change_watcher = io
            .fs
            .watch_for_change(MODS_PATH.as_ref(), Some("state.wasm".as_ref())); // TODO: even tho watching individual files makes more sense, it should still make sure it's the same the server watches

        let cache_clone = cache.clone();
        let task = io.io_batcher.spawn(async move {
            cache_clone
                .load(&wasm_path_str, |wasm_bytes| {
                    Ok(WasmManager::compile_module(&wasm_bytes[..])?
                        .serialize()?
                        .to_vec())
                })
                .await
        });
        let (state, info) = if let Ok(wasm_module) = task.get_storage() {
            let mut info = GameStateStaticInfo {
                ticks_in_a_second: Default::default(),
            };
            let state = StateWasm::new(map, options, &wasm_module, &mut info);
            (GameStateWrapper::Wasm(state), info)
        } else {
            let is_ddnet = true;
            /*if is_ddnet {
                let (state, info) = <Ddnet as GameStateCreate>::new(map, options);
                (GameStateWrapper::Ddnet(state), info)
            } else */
            {
                let (state, info) = GameState::new(map, options);
                (GameStateWrapper::Native(state), info)
            }
        };
        Self {
            state,
            fs_change_watcher,

            info,

            predicted_game_monotonic_tick: 0,
        }
    }

    pub fn should_reload(&self) -> bool {
        self.fs_change_watcher.has_file_change()
    }

    pub fn game_tick_speed(&self) -> GameTickType {
        self.info.ticks_in_a_second
    }
}

impl GameStateCreate for GameStateWasmManager {
    fn new(_map: Vec<u8>, _options: GameStateCreateOptions) -> (Self, GameStateStaticInfo)
    where
        Self: Sized,
    {
        panic!("intentionally not implemented for this type.")
    }
}

impl GameStateInterface for GameStateWasmManager {
    fn all_projectiles(&self, ratio: f64) -> PoolVec<ProjectileRenderInfo> {
        self.state.as_ref().all_projectiles(ratio)
    }

    fn all_ctf_flags(&self, ratio: f64) -> PoolVec<FlagRenderInfo> {
        self.state.as_ref().all_ctf_flags(ratio)
    }

    fn all_lasers(&self, ratio: f64) -> PoolVec<LaserRenderInfo> {
        self.state.as_ref().all_lasers(ratio)
    }

    fn all_pickups(&self, ratio: f64) -> PoolVec<PickupRenderInfo> {
        self.state.as_ref().all_pickups(ratio)
    }

    fn collect_characters_render_info(
        &self,
        intra_tick_ratio: f64,
    ) -> PoolLinkedHashMap<GameEntityId, CharacterRenderInfo> {
        self.state
            .as_ref()
            .collect_characters_render_info(intra_tick_ratio)
    }

    fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {
        self.state.as_ref().collect_characters_info()
    }

    fn collect_scoreboard_info(&self) -> ScoreboardGameType {
        self.state.as_ref().collect_scoreboard_info()
    }

    fn collect_character_local_render_info(
        &self,
        player_id: &GameEntityId,
    ) -> LocalCharacterRenderInfo {
        self.state
            .as_ref()
            .collect_character_local_render_info(player_id)
    }

    fn get_client_camera_join_pos(&self) -> vec2 {
        self.state.as_ref().get_client_camera_join_pos()
    }

    fn player_join(&mut self, player_info: &PlayerClientInfo) -> GameEntityId {
        self.state.as_mut().player_join(player_info)
    }

    fn player_drop(&mut self, player_id: &GameEntityId) {
        self.state.as_mut().player_drop(player_id)
    }

    fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand) {
        self.state.as_mut().client_command(player_id, cmd)
    }

    fn try_overwrite_player_character_info(
        &mut self,
        id: &GameEntityId,
        info: &NetworkCharacterInfo,
        version: NonZeroU64,
    ) {
        self.state
            .as_mut()
            .try_overwrite_player_character_info(id, info, version)
    }

    fn set_player_input(
        &mut self,
        player_id: &GameEntityId,
        inp: &CharacterInput,
        diff: CharacterInputConsumableDiff,
    ) {
        self.state.as_mut().set_player_input(player_id, inp, diff)
    }

    fn tick(&mut self) {
        self.state.as_mut().tick();
    }

    fn pred_tick(
        &mut self,
        inps: PoolLinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>,
    ) {
        self.state.as_mut().pred_tick(inps)
    }

    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolVec<u8> {
        self.state.as_ref().snapshot_for(client)
    }

    fn build_from_snapshot(&mut self, snapshot: &MtPoolVec<u8>) -> SnapshotLocalPlayers {
        let local_players = self.state.as_mut().build_from_snapshot(snapshot);
        local_players
    }

    fn events_for(&self, client: EventClientInfo) -> GameEvents {
        self.state.as_ref().events_for(client)
    }

    fn clear_events(&mut self) {
        self.state.as_mut().clear_events()
    }

    fn sync_event_id(&self, event_id: EventId) {
        self.state.as_ref().sync_event_id(event_id)
    }
}
