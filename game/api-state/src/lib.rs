#![allow(dead_code, unused_variables)]
use std::{sync::Arc, time::Duration};

use api_wasm_macros::{host_func_auto_call, host_func_auto_call_dummy, impl_guest_functions};
use base_log::log::SystemLog;
use math::math::vector::{dvec2, vec2};
use pool::mt_datatypes::PoolVec;
use shared_base::{
    game_types::TGameElementID,
    mapdef::{CMapItemLayerTilemap, MapLayerTile, MapTileLayerDetail},
    network::messages::{MsgObjPlayerInfo, MsgObjPlayerInput, MsgSvPlayerInfo},
    types::GameTickType,
};

use shared_game::{
    entities::{
        flag::flag::FlagRenderInfo, laser::laser::LaserRenderInfo,
        pickup::pickup::PickupRenderInfo, projectile::projectile::ProjectileRenderInfo,
    },
    player::player::{PlayerInput, PlayerRenderInfo},
    snapshot::snapshot::{Snapshot, SnapshotClientInfo},
    state::state::{GameStateCreateOptions, GameStateCreatePipe, GameStateInterface},
};

use api::read_param_from_host;
use api::read_param_from_host_ex;
use api::upload_return_val;

extern "Rust" {
    /// returns an instance of the game state and the game tick speed
    fn mod_state_new(
        create_pipe: &GameStateCreatePipe,
        log: &Arc<SystemLog>,
        options: &GameStateCreateOptions,
    ) -> (Box<dyn GameStateInterface>, GameTickType);
}

pub struct APIState {
    state: Box<dyn GameStateInterface>,
    game_tick_speed: GameTickType,
}

static mut SYS: once_cell::unsync::Lazy<Arc<SystemLog>> =
    once_cell::unsync::Lazy::new(|| Arc::new(SystemLog::new()));

static mut API_STATE: once_cell::unsync::Lazy<APIState> = once_cell::unsync::Lazy::new(|| {
    let (state, game_tick_speed) = unsafe {
        mod_state_new(
            &GameStateCreatePipe {
                game_layer: &MapLayerTile {
                    0: CMapItemLayerTilemap::default(),
                    1: MapTileLayerDetail::Tile(),
                    2: Default::default(),
                },
                cur_time: Duration::ZERO,
            },
            &SYS,
            &Default::default(),
        )
    };
    APIState {
        state,
        game_tick_speed,
    }
});

#[impl_guest_functions]
impl APIState {
    #[host_func_auto_call_dummy]
    fn api_game_tick_speed(&self) -> GameTickType {
        let res = self.game_tick_speed;
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_all_players_info(&self) -> Vec<(TGameElementID, MsgObjPlayerInfo)> {
        let mut res = Default::default();
        self.state.all_players_info(&mut res);
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_players_inputs(&self) -> Vec<(TGameElementID, PlayerInput)> {
        let mut res: Vec<(TGameElementID, PlayerInput)> = Default::default();
        self.state.players_inputs(&mut res);
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_all_projectiles(&self) -> Vec<ProjectileRenderInfo> {
        let intra_tick_ratio: f64 = read_param_from_host(0);
        let mut res = Default::default();
        self.state.all_projectiles(intra_tick_ratio, &mut res);
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_all_ctf_flags(&self) -> Vec<FlagRenderInfo> {
        let intra_tick_ratio: f64 = read_param_from_host(0);
        let mut res = Default::default();
        self.state.all_ctf_flags(intra_tick_ratio, &mut res);
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_all_lasers(&self) -> Vec<LaserRenderInfo> {
        let intra_tick_ratio: f64 = read_param_from_host(0);
        let mut res = Default::default();
        self.state.all_lasers(intra_tick_ratio, &mut res);
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_all_pickups(&self) -> Vec<PickupRenderInfo> {
        let intra_tick_ratio: f64 = read_param_from_host(0);
        let mut res = Default::default();
        self.state.all_pickups(intra_tick_ratio, &mut res);
        upload_return_val(res);
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_get_player_and_no_char_player_infos(&self) -> Vec<MsgSvPlayerInfo> {
        let mut res = PoolVec::<MsgSvPlayerInfo>::new_without_pool();
        self.state.get_player_and_no_char_player_infos(&mut res);
        upload_return_val(res.take());
        Default::default()
    }

    #[host_func_auto_call_dummy]
    fn api_collect_players_render_info(&self) -> Vec<PlayerRenderInfo> {
        let intra_tick_ratio: f64 = read_param_from_host(0);
        let mut render_infos = Default::default();
        self.state
            .collect_players_render_info(intra_tick_ratio, &mut render_infos);
        upload_return_val(render_infos);
        Default::default()
    }
}

impl APIState {
    fn new(&mut self) {
        let game_layer: MapLayerTile = read_param_from_host(0);
        let start_time: Duration = read_param_from_host(1);
        let options: GameStateCreateOptions = read_param_from_host(2);
        let (state, game_tick_speed) = unsafe {
            mod_state_new(
                &GameStateCreatePipe {
                    game_layer: &game_layer,
                    cur_time: start_time,
                },
                &SYS,
                &options,
            )
        };
        self.state = state;
        self.game_tick_speed = game_tick_speed;
    }
}

#[no_mangle]
pub fn game_state_new() {
    unsafe {
        API_STATE.new();
    };
}

#[impl_guest_functions]
impl GameStateInterface for APIState {
    #[host_func_auto_call]
    fn lerp_core_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {}

    #[host_func_auto_call]
    fn lerp_core_vel(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {}

    #[host_func_auto_call]
    fn lerp_core_hook_pos(&self, player_id: &TGameElementID, ratio: f64) -> vec2 {}

    #[host_func_auto_call]
    fn cursor_vec2(&self, player_id: &TGameElementID) -> dvec2 {}

    #[host_func_auto_call]
    fn input_dir(&self, player_id: &TGameElementID) -> i32 {}

    #[host_func_auto_call]
    fn set_cur_monotonic_tick(&mut self, cur_monotonic_tick: GameTickType) {}

    #[host_func_auto_call]
    fn first_player_id(&self) -> Option<TGameElementID> {}

    #[host_func_auto_call]
    fn player_id_after_id(&self, id: &TGameElementID) -> Option<TGameElementID> {}

    #[host_func_auto_call]
    fn add_stage(&mut self) -> TGameElementID {}

    #[host_func_auto_call]
    fn stage_count(&self) -> usize {}

    #[host_func_auto_call]
    fn generate_next_id(&mut self) -> TGameElementID {}

    #[host_func_auto_call]
    fn player_join(&mut self, player_info: &MsgObjPlayerInfo) -> TGameElementID {}

    #[host_func_auto_call]
    fn try_player_drop(&mut self, player_id: &TGameElementID) {}

    fn all_players_info(&self, pool: &mut Vec<(TGameElementID, MsgObjPlayerInfo)>) {
        panic!("uses a wrapper function instead")
    }

    fn players_inputs(&self, pool: &mut Vec<(TGameElementID, PlayerInput)>) {
        panic!("uses a wrapper function instead")
    }

    fn all_projectiles(&self, ratio: f64, pool: &mut Vec<ProjectileRenderInfo>) {
        panic!("uses a wrapper function instead")
    }

    fn all_ctf_flags(&self, ratio: f64, pool: &mut Vec<FlagRenderInfo>) {
        panic!("uses a wrapper function instead")
    }

    fn all_lasers(&self, ratio: f64, pool: &mut Vec<LaserRenderInfo>) {
        panic!("uses a wrapper function instead")
    }

    fn all_pickups(&self, ratio: f64, pool: &mut Vec<PickupRenderInfo>) {
        panic!("uses a wrapper function instead")
    }

    #[host_func_auto_call]
    fn player_exists(&self, player_id: &TGameElementID) -> bool {}

    fn get_player_and_no_char_player_infos(&self, writer: &mut PoolVec<MsgSvPlayerInfo>) {
        panic!("uses a wrapper function instead")
    }

    fn collect_players_render_info(
        &self,
        intra_tick_ratio: f64,
        render_infos: &mut Vec<PlayerRenderInfo>,
    ) {
        panic!("uses a wrapper function instead")
    }

    #[host_func_auto_call]
    fn get_client_camera_start_pos(&self) -> vec2 {}

    #[host_func_auto_call]
    fn try_overwrite_player_info(
        &mut self,
        id: &TGameElementID,
        info: &MsgObjPlayerInfo,
        version: u64,
    ) {
    }

    #[host_func_auto_call]
    fn set_player_inp(
        &mut self,
        player_id: &TGameElementID,
        inp: &MsgObjPlayerInput,
        version: u64,
        force: bool,
    ) {
    }

    #[host_func_auto_call]
    fn tick(&mut self) -> GameTickType {}

    #[host_func_auto_call]
    fn pred_tick(&mut self) {}

    #[host_func_auto_call]
    fn build_for(&self, client: SnapshotClientInfo) -> Snapshot {}

    #[host_func_auto_call]
    fn convert_to_game_state(&mut self, snapshot: &Snapshot) -> bool {}
}
