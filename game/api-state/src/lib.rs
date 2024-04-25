#![allow(clippy::all)]
#![allow(dead_code, unused_variables)]
use std::num::NonZeroU64;
use std::sync::Arc;

use api_wasm_macros::{guest_func_call_from_host_auto, impl_guest_functions_state};
use base_log::log::SystemLog;
use game_interface::client_commands::ClientCommand;
use game_interface::events::{EventClientInfo, EventId, GameEvents};
use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
use game_interface::types::character_info::NetworkCharacterInfo;
use game_interface::types::game::GameEntityId;
use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
use game_interface::types::player_info::PlayerClientInfo;
use game_interface::types::render::character::CharacterInfo;
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
use pool::datatypes::{PoolLinkedHashMap, PoolVec};
use pool::mt_datatypes::PoolVec as MtPoolVec;

use api::read_param_from_host_ex;
use api::upload_return_val;
use api::{read_param_from_host, RUNTIME_THREAD_POOL};
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

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_state_new(
        map: Vec<u8>,
        options: GameStateCreateOptions,
    ) -> (Box<dyn GameStateInterface>, GameStateStaticInfo);
}

pub struct APIState {
    state: Box<dyn GameStateInterface>,
    info: GameStateStaticInfo,
}

static SYS: once_cell::sync::Lazy<Arc<SystemLog>> =
    once_cell::sync::Lazy::new(|| Arc::new(SystemLog::new()));

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
    let (state, info) = unsafe { mod_state_new(map_bytes, Default::default()) };
    APIState { state, info }
});

impl APIState {
    fn new(&mut self) {
        let map: Vec<u8> = read_param_from_host(0);
        let options: GameStateCreateOptions = read_param_from_host(1);
        let (state, info) = unsafe { mod_state_new(map, options) };
        self.state = state;
        self.info = info;
    }
}

#[no_mangle]
pub fn game_state_new() {
    unsafe {
        API_STATE.new();
        upload_return_val(&API_STATE.info);
    };
}

impl GameStateCreate for APIState {
    fn new(_map: Vec<u8>, _options: GameStateCreateOptions) -> (Self, GameStateStaticInfo)
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
    fn player_drop(&mut self, player_id: &GameEntityId) {}

    #[guest_func_call_from_host_auto]
    fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand) {}

    #[guest_func_call_from_host_auto]
    fn all_projectiles(&self, ratio: f64) -> PoolVec<ProjectileRenderInfo> {}

    #[guest_func_call_from_host_auto]
    fn all_ctf_flags(&self, ratio: f64) -> PoolVec<FlagRenderInfo> {}

    #[guest_func_call_from_host_auto]
    fn all_lasers(&self, ratio: f64) -> PoolVec<LaserRenderInfo> {}

    #[guest_func_call_from_host_auto]
    fn all_pickups(&self, ratio: f64) -> PoolVec<PickupRenderInfo> {}

    #[guest_func_call_from_host_auto]
    fn collect_characters_render_info(
        &self,
        intra_tick_ratio: f64,
    ) -> PoolLinkedHashMap<GameEntityId, CharacterRenderInfo> {
    }

    #[guest_func_call_from_host_auto]
    fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {}

    #[guest_func_call_from_host_auto]
    fn collect_scoreboard_info(&self) -> ScoreboardGameType {}

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
    fn tick(&mut self) {}

    #[guest_func_call_from_host_auto]
    fn pred_tick(
        &mut self,
        inps: PoolLinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>,
    ) {
    }

    #[guest_func_call_from_host_auto]
    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolVec<u8> {}

    #[guest_func_call_from_host_auto]
    fn build_from_snapshot(&mut self, snapshot: &MtPoolVec<u8>) -> SnapshotLocalPlayers {}

    #[guest_func_call_from_host_auto]
    fn events_for(&self, client: EventClientInfo) -> GameEvents {}

    #[guest_func_call_from_host_auto]
    fn clear_events(&mut self) {}

    #[guest_func_call_from_host_auto]
    fn sync_event_id(&self, event_id: EventId) {}
}
