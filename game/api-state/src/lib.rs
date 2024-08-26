#![allow(dead_code, unused_variables)]
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Duration;

use api_wasm_macros::{guest_func_call_from_host_auto, impl_guest_functions_state};
use base_io::io_batcher::IoBatcher;
use game_database::traits::DbInterface;
use game_interface::client_commands::ClientCommand;
use game_interface::events::{EventClientInfo, EventId, GameEvents};
use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
use game_interface::types::character_info::NetworkCharacterInfo;
use game_interface::types::emoticons::EmoticonType;
use game_interface::types::game::GameEntityId;
use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
use game_interface::types::network_stats::PlayerNetworkStats;
use game_interface::types::player_info::{PlayerClientInfo, PlayerDropReason};
use game_interface::types::render::character::{CharacterInfo, TeeEye};
use game_interface::types::render::scoreboard::Scoreboard;
use game_interface::types::render::stage::StageRenderInfo;
use map::map::animations::Animations;
use map::map::config::Config;
use map::map::groups::layers::physics::{MapLayerPhysics, MapLayerTilePhysicsBase};
use map::map::groups::layers::tiles::{TileBase, TileFlags};
use map::map::groups::{MapGroupPhysics, MapGroupPhysicsAttr, MapGroups};
use map::map::metadata::Metadata;
use map::map::resources::Resources;
use map::map::Map;
use map::types::NonZeroU16MinusOne;
use math::math::vector::vec2;
use pool::datatypes::PoolLinkedHashMap;
use pool::mt_datatypes::PoolCow as MtPoolCow;

use api::read_param_from_host_ex;
use api::upload_return_val;
use api::{read_param_from_host, RUNTIME_THREAD_POOL};
use game_interface::{
    interface::GameStateInterface,
    types::{
        render::character::LocalCharacterRenderInfo,
        snapshot::{SnapshotClientInfo, SnapshotLocalPlayers},
    },
};

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_state_new(
        map: Vec<u8>,
        map_name: String,
        options: GameStateCreateOptions,
    ) -> (Box<dyn GameStateInterface>, GameStateStaticInfo);
}

pub struct APIState {
    state: Box<dyn GameStateInterface>,
    info: GameStateStaticInfo,
}

static mut API_STATE: once_cell::unsync::Lazy<APIState> = once_cell::unsync::Lazy::new(|| {
    let map = Map {
        resources: Resources {
            image_arrays: Vec::new(),
            images: Vec::new(),
            sounds: Vec::new(),
        },
        groups: MapGroups {
            background: Vec::new(),
            foreground: Vec::new(),
            physics: MapGroupPhysics {
                attr: MapGroupPhysicsAttr {
                    width: NonZeroU16MinusOne::new(1).unwrap(),
                    height: NonZeroU16MinusOne::new(1).unwrap(),
                },
                layers: vec![MapLayerPhysics::Game(MapLayerTilePhysicsBase {
                    tiles: vec![TileBase {
                        index: 0,
                        flags: TileFlags::empty(),
                    }],
                })],
            },
        },
        animations: Animations {
            pos: Vec::new(),
            color: Vec::new(),
            sound: Vec::new(),
        },
        config: Config {
            commands: Default::default(),
        },
        meta: Metadata {
            authors: Default::default(),
            licenses: Default::default(),
            version: Default::default(),
            credits: Default::default(),
            memo: Default::default(),
        },
    };
    let mut map_bytes = Vec::new();
    map.write(&mut map_bytes, &RUNTIME_THREAD_POOL).unwrap();
    let (state, info) = unsafe { mod_state_new(map_bytes, Default::default(), Default::default()) };
    APIState { state, info }
});

impl APIState {
    fn create(&mut self, map: Vec<u8>, map_name: String, options: GameStateCreateOptions) {
        let (state, info) = unsafe { mod_state_new(map, map_name, options) };
        self.state = state;
        self.info = info;
    }
}

#[no_mangle]
pub fn game_state_new() {
    unsafe {
        let map: Vec<u8> = read_param_from_host(0);
        let map_name: String = read_param_from_host(1);
        let options: GameStateCreateOptions = read_param_from_host(2);
        API_STATE.create(map, map_name, options);
        upload_return_val(&API_STATE.info);
    };
}

impl GameStateCreate for APIState {
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

#[impl_guest_functions_state]
impl GameStateInterface for APIState {
    #[guest_func_call_from_host_auto]
    fn player_join(&mut self, player_info: &PlayerClientInfo) -> GameEntityId {}

    #[guest_func_call_from_host_auto]
    fn player_drop(&mut self, player_id: &GameEntityId, reason: PlayerDropReason) {}

    #[guest_func_call_from_host_auto]
    fn network_stats(&mut self, stats: PoolLinkedHashMap<GameEntityId, PlayerNetworkStats>) {}

    #[guest_func_call_from_host_auto]
    fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand) {}

    #[guest_func_call_from_host_auto]
    fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {}

    #[guest_func_call_from_host_auto]
    fn collect_scoreboard_info(&self) -> Scoreboard {}

    #[guest_func_call_from_host_auto]
    fn all_stages(&self, ratio: f64) -> PoolLinkedHashMap<GameEntityId, StageRenderInfo> {}

    #[guest_func_call_from_host_auto]
    fn collect_character_local_render_info(
        &self,
        player_id: &GameEntityId,
    ) -> LocalCharacterRenderInfo {
    }

    #[guest_func_call_from_host_auto]
    fn get_client_camera_join_pos(&self) -> vec2 {}

    #[guest_func_call_from_host_auto]
    fn try_overwrite_player_character_info(
        &mut self,
        id: &GameEntityId,
        info: &NetworkCharacterInfo,
        version: NonZeroU64,
    ) {
    }

    #[guest_func_call_from_host_auto]
    fn set_player_input(
        &mut self,
        player_id: &GameEntityId,
        inp: &CharacterInput,
        diff: CharacterInputConsumableDiff,
    ) {
    }

    #[guest_func_call_from_host_auto]
    fn set_player_emoticon(&mut self, player_id: &GameEntityId, emoticon: EmoticonType) {}

    #[guest_func_call_from_host_auto]
    fn set_player_eye(&mut self, player_id: &GameEntityId, eye: TeeEye, duration: Duration) {}

    #[guest_func_call_from_host_auto]
    fn tick(&mut self) {}

    #[guest_func_call_from_host_auto]
    fn pred_tick(
        &mut self,
        inps: PoolLinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>,
    ) {
    }

    #[guest_func_call_from_host_auto]
    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolCow<'static, [u8]> {}

    #[guest_func_call_from_host_auto]
    fn build_from_snapshot(&mut self, snapshot: &MtPoolCow<'static, [u8]>) -> SnapshotLocalPlayers {
    }

    #[guest_func_call_from_host_auto]
    fn snapshot_for_hotreload(&self) -> Option<MtPoolCow<'static, [u8]>> {}

    #[guest_func_call_from_host_auto]
    fn build_from_snapshot_by_hotreload(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {}

    #[guest_func_call_from_host_auto]
    fn build_from_snapshot_for_pred(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {}

    #[guest_func_call_from_host_auto]
    fn events_for(&self, client: EventClientInfo) -> GameEvents {}

    #[guest_func_call_from_host_auto]
    fn clear_events(&mut self) {}

    #[guest_func_call_from_host_auto]
    fn sync_event_id(&self, event_id: EventId) {}
}
