#![allow(dead_code, unused_variables)]
use std::cell::RefCell;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use api_wasm_macros::{guest_func_call_from_host_auto, impl_guest_functions_state};
use base_io::io_batcher::IoBatcher;
use game_database::traits::DbInterface;
use game_interface::client_commands::ClientCommand;
use game_interface::events::{EventClientInfo, GameEvents};
use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
use game_interface::types::character_info::NetworkCharacterInfo;
use game_interface::types::emoticons::EmoticonType;
use game_interface::types::id_gen::IdGeneratorIdType;
use game_interface::types::id_types::{CharacterId, PlayerId, StageId};
use game_interface::types::input::CharacterInputInfo;
use game_interface::types::network_stats::PlayerNetworkStats;
use game_interface::types::player_info::{AccountId, Hash, PlayerClientInfo, PlayerDropReason};
use game_interface::types::render::character::{CharacterInfo, TeeEye};
use game_interface::types::render::scoreboard::Scoreboard;
use game_interface::types::render::stage::StageRenderInfo;
use game_interface::types::snapshot::FromSnapshotBuildMode;
use game_interface::types::ticks::TickOptions;
use game_interface::vote_commands::VoteCommand;
use math::math::vector::vec2;
use pool::datatypes::{PoolLinkedHashMap, PoolVec};
use pool::mt_datatypes::PoolCow as MtPoolCow;

use api::read_param_from_host;
use api::read_param_from_host_ex;
use api::upload_return_val;
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
    ) -> Result<(Box<dyn GameStateInterface>, GameStateStaticInfo), String>;
}

pub struct ApiState {
    state: Rc<RefCell<Option<Box<dyn GameStateInterface>>>>,
    info: RefCell<Option<GameStateStaticInfo>>,
}

thread_local! {
static API_STATE: once_cell::unsync::Lazy<ApiState> = once_cell::unsync::Lazy::new(|| ApiState { state: Default::default(), info: Default::default() });
}

impl ApiState {
    fn create(
        &self,
        map: Vec<u8>,
        map_name: String,
        options: GameStateCreateOptions,
    ) -> Result<(), String> {
        let (state, info) = unsafe { mod_state_new(map, map_name, options)? };
        *self.state.borrow_mut() = Some(state);
        *self.info.borrow_mut() = Some(info);
        Ok(())
    }
}

#[no_mangle]
pub fn game_state_new() {
    let map: Vec<u8> = read_param_from_host(0);
    let map_name: String = read_param_from_host(1);
    let options: GameStateCreateOptions = read_param_from_host(2);
    let res = API_STATE.with(|g| g.create(map, map_name, options));
    upload_return_val(API_STATE.with(|g| res.map(|_| g.info.borrow().clone().unwrap())));
}

#[no_mangle]
pub fn game_state_drop() {
    API_STATE.with(|g| *g.state.borrow_mut() = None);
}

impl GameStateCreate for ApiState {
    fn new(
        _map: Vec<u8>,
        _map_name: String,
        _options: GameStateCreateOptions,
        _io_batcher: IoBatcher,
        _db: Arc<dyn DbInterface>,
    ) -> Result<(Self, GameStateStaticInfo), String>
    where
        Self: Sized,
    {
        panic!("intentionally not implemented for this type.")
    }
}

#[impl_guest_functions_state]
impl GameStateInterface for ApiState {
    #[guest_func_call_from_host_auto(option)]
    fn player_join(&mut self, player_info: &PlayerClientInfo) -> PlayerId {}

    #[guest_func_call_from_host_auto(option)]
    fn player_drop(&mut self, player_id: &PlayerId, reason: PlayerDropReason) {}

    #[guest_func_call_from_host_auto(option)]
    fn try_overwrite_player_character_info(
        &mut self,
        id: &PlayerId,
        info: &NetworkCharacterInfo,
        version: NonZeroU64,
    ) {
    }

    #[guest_func_call_from_host_auto(option)]
    fn account_created(&mut self, account_id: AccountId, cert_fingerprint: Hash) {}

    #[guest_func_call_from_host_auto(option)]
    fn network_stats(&mut self, stats: PoolLinkedHashMap<PlayerId, PlayerNetworkStats>) {}

    #[guest_func_call_from_host_auto(option)]
    fn client_command(&mut self, player_id: &PlayerId, cmd: ClientCommand) {}

    #[guest_func_call_from_host_auto(option)]
    fn vote_command(&mut self, cmd: VoteCommand) {}

    #[guest_func_call_from_host_auto(option)]
    fn collect_characters_info(&self) -> PoolLinkedHashMap<CharacterId, CharacterInfo> {}

    #[guest_func_call_from_host_auto(option)]
    fn collect_render_ext(&self) -> PoolVec<u8> {}

    #[guest_func_call_from_host_auto(option)]
    fn collect_scoreboard_info(&self) -> Scoreboard {}

    #[guest_func_call_from_host_auto(option)]
    fn all_stages(&self, ratio: f64) -> PoolLinkedHashMap<StageId, StageRenderInfo> {}

    #[guest_func_call_from_host_auto(option)]
    fn collect_character_local_render_info(
        &self,
        player_id: &PlayerId,
    ) -> LocalCharacterRenderInfo {
    }

    #[guest_func_call_from_host_auto(option)]
    fn get_client_camera_join_pos(&self) -> vec2 {}

    #[guest_func_call_from_host_auto(option)]
    fn set_player_inputs(&mut self, inps: PoolLinkedHashMap<PlayerId, CharacterInputInfo>) {}

    #[guest_func_call_from_host_auto(option)]
    fn set_player_emoticon(&mut self, player_id: &PlayerId, emoticon: EmoticonType) {}

    #[guest_func_call_from_host_auto(option)]
    fn set_player_eye(&mut self, player_id: &PlayerId, eye: TeeEye, duration: Duration) {}

    #[guest_func_call_from_host_auto(option)]
    fn tick(&mut self, options: TickOptions) {}

    #[guest_func_call_from_host_auto(option)]
    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolCow<'static, [u8]> {}

    #[guest_func_call_from_host_auto(option)]
    fn build_from_snapshot(
        &mut self,
        snapshot: &MtPoolCow<'static, [u8]>,
        mode: FromSnapshotBuildMode,
    ) -> SnapshotLocalPlayers {
    }

    #[guest_func_call_from_host_auto(option)]
    fn snapshot_for_hotreload(&self) -> Option<MtPoolCow<'static, [u8]>> {}

    #[guest_func_call_from_host_auto(option)]
    fn build_from_snapshot_by_hotreload(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {}

    #[guest_func_call_from_host_auto(option)]
    fn build_from_snapshot_for_prev(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {}

    #[guest_func_call_from_host_auto(option)]
    fn events_for(&self, client: EventClientInfo) -> GameEvents {}

    #[guest_func_call_from_host_auto(option)]
    fn clear_events(&mut self) {}

    #[guest_func_call_from_host_auto(option)]
    fn sync_event_id(&self, event_id: IdGeneratorIdType) {}
}
