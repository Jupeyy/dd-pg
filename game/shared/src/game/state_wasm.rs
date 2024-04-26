use api_wasm_macros::wasm_mod_prepare_state;

#[wasm_mod_prepare_state]
pub mod state_wasm {
    use std::num::NonZeroU64;

    use anyhow::anyhow;
    use api_wasm_macros::wasm_func_auto_call;
    use game_interface::client_commands::ClientCommand;
    use game_interface::events::{EventClientInfo, EventId, GameEvents};
    use game_interface::interface::{GameStateCreate, GameStateCreateOptions, GameStateStaticInfo};
    use game_interface::types::character_info::NetworkCharacterInfo;
    use game_interface::types::game::GameEntityId;
    use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
    use game_interface::types::player_info::PlayerClientInfo;
    use game_interface::types::render::character::CharacterInfo;
    use math::math::vector::vec2;
    use pool::datatypes::{PoolLinkedHashMap, PoolVec};
    use pool::mt_datatypes::PoolVec as MtPoolVec;
    use wasm_runtime::{WasmManager, WasmManagerModuleType};
    use wasmer::Module;

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

    pub struct StateWasm {
        wasm_manager: WasmManager,
    }

    #[constructor]
    impl StateWasm {
        pub fn new(
            map: Vec<u8>,
            options: GameStateCreateOptions,
            wasm_module: &Vec<u8>,
            info: &mut GameStateStaticInfo,
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
            wasm_manager.add_param(0, &map);
            wasm_manager.add_param(1, &options);
            wasm_manager.run_by_name::<()>("game_state_new").unwrap();
            *info = wasm_manager.get_result_as();

            Self { wasm_manager }
        }
    }

    impl GameStateCreate for StateWasm {
        fn new(_map: Vec<u8>, _options: GameStateCreateOptions) -> (Self, GameStateStaticInfo)
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
        fn player_drop(&mut self, player_id: &GameEntityId) {}

        #[wasm_func_auto_call]
        fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand) {}

        #[wasm_func_auto_call]
        fn all_projectiles(&self, ratio: f64) -> PoolVec<ProjectileRenderInfo> {}

        #[wasm_func_auto_call]
        fn all_ctf_flags(&self, ratio: f64) -> PoolVec<FlagRenderInfo> {}

        #[wasm_func_auto_call]
        fn all_lasers(&self, ratio: f64) -> PoolVec<LaserRenderInfo> {}

        #[wasm_func_auto_call]
        fn all_pickups(&self, ratio: f64) -> PoolVec<PickupRenderInfo> {}

        #[wasm_func_auto_call]
        fn collect_characters_render_info(
            &self,
            intra_tick_ratio: f64,
        ) -> PoolLinkedHashMap<GameEntityId, CharacterRenderInfo> {
        }

        #[wasm_func_auto_call]
        fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo> {}

        #[wasm_func_auto_call]
        fn collect_scoreboard_info(&self) -> ScoreboardGameType {}

        #[wasm_func_auto_call]
        fn collect_character_local_render_info(
            &self,
            player_id: &GameEntityId,
        ) -> LocalCharacterRenderInfo {
        }

        #[wasm_func_auto_call]
        fn get_client_camera_join_pos(&self) -> vec2 {}

        #[wasm_func_auto_call]
        fn try_overwrite_player_character_info(
            &mut self,
            id: &GameEntityId,
            info: &NetworkCharacterInfo,
            version: NonZeroU64,
        ) {
        }

        #[wasm_func_auto_call]
        fn set_player_input(
            &mut self,
            player_id: &GameEntityId,
            inp: &CharacterInput,
            diff: CharacterInputConsumableDiff,
        ) {
        }

        #[wasm_func_auto_call]
        fn tick(&mut self) {}

        #[wasm_func_auto_call]
        fn pred_tick(
            &mut self,
            inps: PoolLinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>,
        ) {
        }

        #[wasm_func_auto_call]
        fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolVec<u8> {}

        #[wasm_func_auto_call]
        fn build_from_snapshot(&mut self, snapshot: &MtPoolVec<u8>) -> SnapshotLocalPlayers {}

        #[wasm_func_auto_call]
        fn events_for(&self, client: EventClientInfo) -> GameEvents {}

        #[wasm_func_auto_call]
        fn clear_events(&mut self) {}

        #[wasm_func_auto_call]
        fn sync_event_id(&self, event_id: EventId) {}
    }
}
