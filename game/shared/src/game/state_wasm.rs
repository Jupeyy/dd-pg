use api_wasm_macros::wasm_func_state_prepare;

#[wasm_func_state_prepare]
pub mod state_wasm {
    use anyhow::anyhow;
    use api_wasm_macros::wasm_func_auto_call;
    use math::math::vector::vec2;
    use wasm_runtime::{WasmManager, WasmManagerModuleType};
    use wasmer::Module;

    use shared_base::{
        game_types::TGameElementID,
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

    pub struct StateWasm {
        wasm_manager: WasmManager,

        api_game_tick_speed_name: wasmer::TypedFunction<(), ()>,

        api_all_players_info_name: wasmer::TypedFunction<(), ()>,
        api_players_inputs_name: wasmer::TypedFunction<(), ()>,

        api_all_projectiles_name: wasmer::TypedFunction<(), ()>,
        api_all_ctf_flags_name: wasmer::TypedFunction<(), ()>,
        api_all_lasers_name: wasmer::TypedFunction<(), ()>,
        api_all_pickups_name: wasmer::TypedFunction<(), ()>,

        api_get_player_and_no_char_player_infos_name: wasmer::TypedFunction<(), ()>,
        api_collect_players_render_info_name: wasmer::TypedFunction<(), ()>,
    }

    #[constructor]
    impl StateWasm {
        pub fn new(
            create_pipe: &GameStateCreatePipe,
            options: &GameStateCreateOptions,
            wasm_module: &Vec<u8>,
        ) -> Self {
            let wasm_manager = WasmManager::new(
                WasmManagerModuleType::FromClosure(|store| {
                    match unsafe { Module::deserialize(store, wasm_module.as_slice()) } {
                        Ok(module) => Ok(module),
                        Err(err) => Err(anyhow!(err)),
                    }
                }),
                |_store, _func_env| None,
            )
            .unwrap();
            wasm_manager.add_param(0, create_pipe.game_layer);
            wasm_manager.add_param(1, &create_pipe.cur_time);
            wasm_manager.add_param(2, options);
            wasm_manager.run_by_name("game_state_new").unwrap();

            let api_game_tick_speed_name = wasm_manager.run_func_by_name("api_game_tick_speed");

            let api_all_players_info_name = wasm_manager.run_func_by_name("api_all_players_info");
            let api_players_inputs_name = wasm_manager.run_func_by_name("api_players_inputs");

            let api_all_projectiles_name = wasm_manager.run_func_by_name("api_all_projectiles");
            let api_all_ctf_flags_name = wasm_manager.run_func_by_name("api_all_ctf_flags");
            let api_all_lasers_name = wasm_manager.run_func_by_name("api_all_lasers");
            let api_all_pickups_name = wasm_manager.run_func_by_name("api_all_pickups");

            let api_get_player_and_no_char_player_infos_name =
                wasm_manager.run_func_by_name("api_get_player_and_no_char_player_infos");
            let api_collect_players_render_info_name =
                wasm_manager.run_func_by_name("api_collect_players_render_info");
            Self {
                wasm_manager,

                api_game_tick_speed_name,

                api_all_players_info_name,
                api_players_inputs_name,

                api_all_projectiles_name,
                api_all_ctf_flags_name,
                api_all_lasers_name,
                api_all_pickups_name,

                api_get_player_and_no_char_player_infos_name,
                api_collect_players_render_info_name,
            }
        }
    }

    impl StateWasm {
        #[wasm_func_auto_call(no_res)]
        fn api_game_tick_speed(&self) -> GameTickType {}

        pub fn game_tick_speed(&self) -> GameTickType {
            self.api_game_tick_speed();
            self.wasm_manager.get_result_as()
        }

        #[wasm_func_auto_call(no_res)]
        fn api_all_players_info(&self) -> Vec<(TGameElementID, MsgObjPlayerInfo)> {}

        #[wasm_func_auto_call(no_res)]
        fn api_players_inputs(&self) -> Vec<(TGameElementID, PlayerInput)> {}

        #[wasm_func_auto_call(no_res)]
        fn api_all_projectiles(&self, ratio: f64) -> Vec<ProjectileRenderInfo> {}

        #[wasm_func_auto_call(no_res)]
        fn api_all_ctf_flags(&self, ratio: f64) -> Vec<FlagRenderInfo> {}

        #[wasm_func_auto_call(no_res)]
        fn api_all_lasers(&self, ratio: f64) -> Vec<LaserRenderInfo> {}

        #[wasm_func_auto_call(no_res)]
        fn api_all_pickups(&self, ratio: f64) -> Vec<PickupRenderInfo> {}

        #[wasm_func_auto_call(no_res)]
        fn api_get_player_and_no_char_player_infos(&self) -> Vec<MsgSvPlayerInfo> {}

        #[wasm_func_auto_call(no_res)]
        fn api_collect_players_render_info(&self, intra_tick_ratio: f64) -> Vec<PlayerRenderInfo> {}
    }

    impl GameStateInterface for StateWasm {
        #[wasm_func_auto_call]
        fn lerp_core_pos(
            &self,
            player_id: &TGameElementID,
            ratio: f64,
        ) -> math::math::vector::vec2 {
        }

        #[wasm_func_auto_call]
        fn lerp_core_vel(
            &self,
            player_id: &TGameElementID,
            ratio: f64,
        ) -> math::math::vector::vec2 {
        }

        #[wasm_func_auto_call]
        fn lerp_core_hook_pos(
            &self,
            player_id: &TGameElementID,
            ratio: f64,
        ) -> math::math::vector::vec2 {
        }

        #[wasm_func_auto_call]
        fn cursor_vec2(&self, player_id: &TGameElementID) -> math::math::vector::dvec2 {}

        #[wasm_func_auto_call]
        fn input_dir(&self, player_id: &TGameElementID) -> i32 {}

        #[wasm_func_auto_call]
        fn set_cur_monotonic_tick(&mut self, cur_monotonic_tick: GameTickType) {}

        #[wasm_func_auto_call]
        fn first_player_id(&self) -> Option<TGameElementID> {}

        #[wasm_func_auto_call]
        fn player_id_after_id(&self, id: &TGameElementID) -> Option<TGameElementID> {}

        #[wasm_func_auto_call]
        fn add_stage(&mut self) -> TGameElementID {}

        #[wasm_func_auto_call]
        fn stage_count(&self) -> usize {}

        #[wasm_func_auto_call]
        fn generate_next_id(&mut self) -> TGameElementID {}

        #[wasm_func_auto_call]
        fn player_join(&mut self, player_info: &MsgObjPlayerInfo) -> TGameElementID {}

        #[wasm_func_auto_call]
        fn try_player_drop(&mut self, player_id: &TGameElementID) {}

        fn all_players_info(&self, pool: &mut Vec<(TGameElementID, MsgObjPlayerInfo)>) {
            self.api_all_players_info();
            pool.clone_from(&self.wasm_manager.get_result_as());
        }

        fn players_inputs(&self, inps: &mut Vec<(TGameElementID, PlayerInput)>) {
            self.api_players_inputs();
            inps.clone_from(&self.wasm_manager.get_result_as());
        }

        fn all_projectiles(&self, ratio: f64, pool: &mut Vec<ProjectileRenderInfo>) {
            self.api_all_projectiles(ratio);
            pool.clone_from(&self.wasm_manager.get_result_as());
        }

        fn all_ctf_flags(&self, ratio: f64, pool: &mut Vec<FlagRenderInfo>) {
            self.api_all_ctf_flags(ratio);
            pool.clone_from(&self.wasm_manager.get_result_as());
        }

        fn all_lasers(&self, ratio: f64, pool: &mut Vec<LaserRenderInfo>) {
            self.api_all_lasers(ratio);
            pool.clone_from(&self.wasm_manager.get_result_as());
        }

        fn all_pickups(&self, ratio: f64, pool: &mut Vec<PickupRenderInfo>) {
            self.api_all_pickups(ratio);
            pool.clone_from(&self.wasm_manager.get_result_as());
        }

        #[wasm_func_auto_call]
        fn player_exists(&self, player_id: &TGameElementID) -> bool {}

        fn get_player_and_no_char_player_infos(
            &self,
            writer: &mut pool::mt_datatypes::PoolVec<MsgSvPlayerInfo>,
        ) {
            self.api_get_player_and_no_char_player_infos();
            writer.clone_from(&self.wasm_manager.get_result_as());
        }

        fn collect_players_render_info(
            &self,
            intra_tick_ratio: f64,
            render_infos: &mut Vec<PlayerRenderInfo>,
        ) {
            self.api_collect_players_render_info(intra_tick_ratio);
            render_infos.clone_from(&self.wasm_manager.get_result_as());
        }

        #[wasm_func_auto_call]
        fn get_client_camera_start_pos(&self) -> vec2 {}

        #[wasm_func_auto_call]
        fn try_overwrite_player_info(
            &mut self,
            id: &TGameElementID,
            info: &MsgObjPlayerInfo,
            version: u64,
        ) {
        }

        #[wasm_func_auto_call]
        fn set_player_inp(
            &mut self,
            player_id: &TGameElementID,
            inp: &MsgObjPlayerInput,
            version: u64,
            force: bool,
        ) {
        }

        #[wasm_func_auto_call]
        fn tick(&mut self) -> GameTickType {}

        #[wasm_func_auto_call]
        fn pred_tick(&mut self) {}

        #[wasm_func_auto_call]
        fn build_for(&self, client: SnapshotClientInfo) -> Snapshot {}

        #[wasm_func_auto_call]
        fn convert_to_game_state(&mut self, snapshot: &Snapshot) -> bool {}
    }
}
