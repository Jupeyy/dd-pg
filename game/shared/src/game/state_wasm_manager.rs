use std::num::{NonZero, NonZeroU64};
use std::sync::Arc;
use std::time::Duration;

use base_io::io::Io;
use base_io::io_batcher::IoBatcher;
use base_io_traits::fs_traits::FileSystemInterface;
use cache::Cache;
use game_database::traits::DbInterface;
//use ddnet::Ddnet;
use game_interface::client_commands::ClientCommand;
use game_interface::events::{EventClientInfo, EventId, GameEvents};
use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
use game_interface::types::character_info::NetworkCharacterInfo;
use game_interface::types::emoticons::EmoticonType;
use game_interface::types::game::{GameEntityId, GameTickType, NonZeroGameTickType};
use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
use game_interface::types::network_stats::PlayerNetworkStats;
use game_interface::types::player_info::{PlayerClientInfo, PlayerDropReason};
use game_interface::types::render::character::{CharacterInfo, TeeEye};
use game_interface::types::render::scoreboard::Scoreboard;
use game_interface::types::render::stage::StageRenderInfo;
use math::math::vector::vec2;
use pool::datatypes::PoolLinkedHashMap;
use pool::mt_datatypes::PoolCow as MtPoolCow;
use shared_game::state::state::GameState;
use wasm_runtime::WasmManager;

use game_interface::{
    interface::GameStateInterface,
    types::{
        render::character::LocalCharacterRenderInfo,
        snapshot::{SnapshotClientInfo, SnapshotLocalPlayers},
    },
};

use super::state_wasm::state_wasm::StateWasm;

#[derive(Debug, Clone)]
pub enum GameStateMod {
    Native,
    Ddnet,
    Wasm { file: Vec<u8> },
}

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

    pub info: GameStateStaticInfo,

    pub predicted_game_monotonic_tick: GameTickType,
}

pub const STATE_MODS_PATH: &str = "mods/state";

impl GameStateWasmManager {
    pub async fn load_module(
        fs: &Arc<dyn FileSystemInterface>,
        file: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let cache = Arc::new(Cache::<0>::new(STATE_MODS_PATH, fs));
        cache
            .load_from_binary(file, |wasm_bytes| {
                Ok(WasmManager::compile_module(wasm_bytes)?
                    .serialize()?
                    .to_vec())
            })
            .await
    }

    pub fn new(
        game_mod: GameStateMod,
        map: Vec<u8>,
        map_name: String,
        options: GameStateCreateOptions,
        io: &Io,
        db: Arc<dyn DbInterface>,
    ) -> Self {
        let (state, info) = match game_mod {
            GameStateMod::Native => {
                let (state, info) =
                    GameState::new(map, map_name, options, io.io_batcher.clone(), db);
                (GameStateWrapper::Native(state), info)
            }
            GameStateMod::Ddnet => {
                // TODO: let (state, info) = <Ddnet as GameStateCreate>::new(map, options);
                // (GameStateWrapper::Ddnet(state), info)
                let (state, info) =
                    GameState::new(map, map_name, options, io.io_batcher.clone(), db);
                (GameStateWrapper::Native(state), info)
            }
            GameStateMod::Wasm { file: wasm_module } => {
                let mut info = GameStateStaticInfo {
                    ticks_in_a_second: NonZero::new(50).unwrap(),
                    chat_commands: Default::default(),
                    rcon_commands: Default::default(),
                    config: None,
                };
                let state = StateWasm::new(
                    map,
                    map_name,
                    options,
                    &wasm_module,
                    &mut info,
                    io.io_batcher.clone(),
                    db,
                );
                (GameStateWrapper::Wasm(state), info)
            }
        };
        Self {
            state,

            info,

            predicted_game_monotonic_tick: 0,
        }
    }

    /// Never 0
    pub fn game_tick_speed(&self) -> NonZeroGameTickType {
        self.info.ticks_in_a_second
    }
}

impl GameStateCreate for GameStateWasmManager {
    fn new(
        _map: Vec<u8>,
        _map_name: String,
        _options: GameStateCreateOptions,
        _io_batcher: IoBatcher,
        _db: Arc<dyn DbInterface>,
    ) -> (Self, GameStateStaticInfo)
    where
        Self: Sized,
    {
        panic!("intentionally not implemented for this type.")
    }
}

impl GameStateInterface for GameStateWasmManager {
    fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {
        self.state.as_ref().collect_characters_info()
    }

    fn collect_scoreboard_info(&self) -> Scoreboard {
        self.state.as_ref().collect_scoreboard_info()
    }

    fn all_stages(&self, ratio: f64) -> PoolLinkedHashMap<GameEntityId, StageRenderInfo> {
        self.state.as_ref().all_stages(ratio)
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

    fn player_drop(&mut self, player_id: &GameEntityId, reason: PlayerDropReason) {
        self.state.as_mut().player_drop(player_id, reason)
    }

    fn network_stats(&mut self, stats: PoolLinkedHashMap<GameEntityId, PlayerNetworkStats>) {
        self.state.as_mut().network_stats(stats)
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

    fn set_player_emoticon(&mut self, player_id: &GameEntityId, emoticon: EmoticonType) {
        self.state.as_mut().set_player_emoticon(player_id, emoticon)
    }

    fn set_player_eye(&mut self, player_id: &GameEntityId, eye: TeeEye, duration: Duration) {
        self.state.as_mut().set_player_eye(player_id, eye, duration)
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

    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolCow<'static, [u8]> {
        self.state.as_ref().snapshot_for(client)
    }

    fn build_from_snapshot(&mut self, snapshot: &MtPoolCow<'static, [u8]>) -> SnapshotLocalPlayers {
        self.state.as_mut().build_from_snapshot(snapshot)
    }

    fn snapshot_for_hotreload(&self) -> Option<MtPoolCow<'static, [u8]>> {
        self.state.as_ref().snapshot_for_hotreload()
    }

    fn build_from_snapshot_by_hotreload(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {
        self.state
            .as_mut()
            .build_from_snapshot_by_hotreload(snapshot)
    }

    fn build_from_snapshot_for_pred(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {
        self.state.as_mut().build_from_snapshot_for_pred(snapshot)
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
