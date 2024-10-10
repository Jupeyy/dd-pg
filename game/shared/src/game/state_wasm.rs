use api_wasm_macros::wasm_mod_prepare_state;

#[wasm_mod_prepare_state]
pub mod state_wasm {
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::anyhow;
    use api_wasm_macros::wasm_func_auto_call;
    use base::hash::Hash;
    use base_io::io_batcher::IoBatcher;
    use game_database::traits::DbInterface;
    use game_interface::client_commands::ClientCommand;
    use game_interface::events::{EventClientInfo, EventId, GameEvents};
    use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
    use game_interface::types::character_info::NetworkCharacterInfo;
    use game_interface::types::emoticons::EmoticonType;
    use game_interface::types::game::GameEntityId;
    use game_interface::types::input::CharacterInputInfo;
    use game_interface::types::network_stats::PlayerNetworkStats;
    use game_interface::types::player_info::{AccountId, PlayerClientInfo, PlayerDropReason};
    use game_interface::types::render::character::{CharacterInfo, TeeEye};
    use game_interface::types::render::scoreboard::Scoreboard;
    use game_interface::types::render::stage::StageRenderInfo;
    use game_interface::types::snapshot::FromSnapshotBuildMode;
    use game_interface::types::ticks::TickOptions;
    use game_interface::vote_commands::VoteCommand;
    use math::math::vector::vec2;
    use pool::datatypes::PoolLinkedHashMap;
    use pool::mt_datatypes::PoolCow as MtPoolCow;
    use wasm_logic_db::db::WasmDatabaseLogic;
    use wasm_runtime::{WasmManager, WasmManagerModuleType};
    use wasmer::Module;

    use game_interface::{
        interface::GameStateInterface,
        types::{
            render::character::LocalCharacterRenderInfo,
            snapshot::{SnapshotClientInfo, SnapshotLocalPlayers},
        },
    };

    pub struct StateWasm {
        wasm_manager: WasmManager,
    }

    #[constructor]
    impl StateWasm {
        pub fn new(
            map: Vec<u8>,
            map_name: String,
            options: GameStateCreateOptions,
            wasm_module: &Vec<u8>,
            info: &mut GameStateStaticInfo,
            io_batcher: IoBatcher,
            db: Arc<dyn DbInterface>,
        ) -> Self {
            let db_logic = WasmDatabaseLogic::new(io_batcher, db);

            let wasm_manager = WasmManager::new(
                WasmManagerModuleType::FromClosure(|store| {
                    match unsafe { Module::deserialize(store, wasm_module.as_slice()) } {
                        Ok(module) => Ok(module),
                        Err(err) => Err(anyhow!(err)),
                    }
                }),
                |store, raw_bytes_env| {
                    let imports = db_logic.get_wasm_logic_imports(store, raw_bytes_env);
                    Some(imports)
                },
            )
            .unwrap();
            wasm_manager.add_param(0, &map);
            wasm_manager.add_param(1, &map_name);
            wasm_manager.add_param(2, &options);
            wasm_manager.run_by_name::<()>("game_state_new").unwrap();
            *info = wasm_manager.get_result_as();

            Self { wasm_manager }
        }
    }

    impl GameStateCreate for StateWasm {
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

    impl GameStateInterface for StateWasm {
        #[wasm_func_auto_call]
        fn player_join(&mut self, player_info: &PlayerClientInfo) -> GameEntityId {}

        #[wasm_func_auto_call]
        fn player_drop(&mut self, player_id: &GameEntityId, reason: PlayerDropReason) {}

        #[wasm_func_auto_call]
        fn try_overwrite_player_character_info(
            &mut self,
            id: &GameEntityId,
            info: &NetworkCharacterInfo,
            version: NonZeroU64,
        ) {
        }

        #[wasm_func_auto_call]
        fn account_created(&mut self, account_id: AccountId, cert_fingerprint: Hash) {}

        #[wasm_func_auto_call]
        fn network_stats(&mut self, stats: PoolLinkedHashMap<GameEntityId, PlayerNetworkStats>) {}

        #[wasm_func_auto_call]
        fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand) {}

        #[wasm_func_auto_call]
        fn vote_command(&mut self, cmd: VoteCommand) {}

        #[wasm_func_auto_call]
        fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {}

        #[wasm_func_auto_call]
        fn collect_scoreboard_info(&self) -> Scoreboard {}

        #[wasm_func_auto_call]
        fn all_stages(&self, ratio: f64) -> PoolLinkedHashMap<GameEntityId, StageRenderInfo> {}

        #[wasm_func_auto_call]
        fn collect_character_local_render_info(
            &self,
            player_id: &GameEntityId,
        ) -> LocalCharacterRenderInfo {
        }

        #[wasm_func_auto_call]
        fn get_client_camera_join_pos(&self) -> vec2 {}

        #[wasm_func_auto_call]
        fn set_player_inputs(&mut self, inps: PoolLinkedHashMap<GameEntityId, CharacterInputInfo>) {
        }

        #[wasm_func_auto_call]
        fn set_player_emoticon(&mut self, player_id: &GameEntityId, emoticon: EmoticonType) {}

        #[wasm_func_auto_call]
        fn set_player_eye(&mut self, player_id: &GameEntityId, eye: TeeEye, duration: Duration) {}

        #[wasm_func_auto_call]
        fn tick(&mut self, options: TickOptions) {}

        #[wasm_func_auto_call]
        fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolCow<'static, [u8]> {}

        #[wasm_func_auto_call]
        fn build_from_snapshot(
            &mut self,
            snapshot: &MtPoolCow<'static, [u8]>,
            mode: FromSnapshotBuildMode,
        ) -> SnapshotLocalPlayers {
        }

        #[wasm_func_auto_call]
        fn snapshot_for_hotreload(&self) -> Option<MtPoolCow<'static, [u8]>> {}

        #[wasm_func_auto_call]
        fn build_from_snapshot_by_hotreload(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {}

        #[wasm_func_auto_call]
        fn build_from_snapshot_for_prev(&mut self, snapshot: &MtPoolCow<'static, [u8]>) {}

        #[wasm_func_auto_call]
        fn events_for(&self, client: EventClientInfo) -> GameEvents {}

        #[wasm_func_auto_call]
        fn clear_events(&mut self) {}

        #[wasm_func_auto_call]
        fn sync_event_id(&self, event_id: EventId) {}
    }

    impl Drop for StateWasm {
        fn drop(&mut self) {
            self.wasm_manager
                .run_by_name::<()>("game_state_drop")
                .unwrap();
        }
    }
}
