use std::sync::Arc;

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use hashlink::LinkedHashMap;

use network::network::network::NetworkConnectionID;

use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::{
    datafile::{CDatafileWrapper, MapFileLayersReadOptions, MapFileOpenOptions},
    game_types::TGameElementID,
    mapdef::MapItemTypes,
    network::messages::{MsgObjPlayerInfo, MsgObjPlayerInput},
};

use shared_game::state::state::{GameStateCreatePipe, GameStateInterface};

use base::system::{System, SystemTimeInterface};
use thiserror::Error;

pub struct ServerPlayer {
    pub network_id: NetworkConnectionID,
    pub id: TGameElementID,
}

impl ServerPlayer {
    pub fn new(network_id: &NetworkConnectionID, id: &TGameElementID) -> Self {
        Self {
            network_id: *network_id,
            id: id.clone(),
        }
    }
}

pub struct ServerMap {
    pub raw: CDatafileWrapper,
}

#[derive(Error, Debug)]
pub enum ServerMapError {
    #[error("Map file not found")]
    FileNotFound,
}

impl ServerMap {
    pub fn new(
        map_file: &str,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
    ) -> Result<Self, ServerMapError> {
        let map_file_name = map_file.to_string() + &".map";
        let mapfile = std::fs::read("data/maps/".to_string() + map_file_name.as_str());
        if let Ok(map_) = mapfile {
            let mut file_wrap = CDatafileWrapper::new();
            let mut load_options = MapFileOpenOptions::default();
            load_options
                .dont_load_map_item
                .iter_mut()
                .for_each(|i| *i = true);
            load_options.dont_load_map_item[MapItemTypes::Group as usize] = false;
            load_options.dont_load_map_item[MapItemTypes::Layer as usize] = false;
            let res = file_wrap.open(
                &map_,
                &map_file,
                runtime_thread_pool.as_ref(),
                &load_options,
                &sys,
            );
            if let Ok(data_start) = res {
                CDatafileWrapper::read_map_layers(
                    &file_wrap.data_file,
                    &mut file_wrap.layers,
                    data_start,
                    &sys,
                    &MapFileLayersReadOptions {
                        dont_load_design_layers: true,
                        ..Default::default()
                    },
                );
            }
            file_wrap.init_layers(&runtime_thread_pool);

            return Ok(Self { raw: file_wrap });
        }
        Err(ServerMapError::FileNotFound)
    }
}

pub struct ServerGame {
    pub players: LinkedHashMap<TGameElementID, ServerPlayer>,
    pub game: GameStateWasmManager,
    pub map: ServerMap,
}

impl ServerGame {
    pub fn new(
        sys: &System,
        start_map: &str,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        fs: &Arc<FileSystem>,
        io_batcher: &TokIOBatcher,
    ) -> Self {
        let map = ServerMap::new(start_map, runtime_thread_pool, &sys.time).unwrap();
        let game = GameStateWasmManager::new(
            &GameStateCreatePipe {
                game_layer: map.raw.get_game_layer(),
                cur_time: sys.time_get_nanoseconds(),
            },
            &Default::default(),
            sys,
            fs,
            io_batcher,
        );
        Self {
            players: Default::default(),
            game,
            map,
        }
    }

    pub fn player_join(
        &mut self,
        network_id: &NetworkConnectionID,
        player_info: &MsgObjPlayerInfo,
    ) -> TGameElementID {
        let player_id = self.game.player_join(player_info);
        self.players
            .insert(player_id.clone(), ServerPlayer::new(network_id, &player_id));
        player_id
    }

    pub fn try_player_drop(&mut self, player_id: &TGameElementID) {
        self.game.try_player_drop(player_id)
    }

    pub fn player_inp(
        &mut self,
        player_id: &TGameElementID,
        inp: &MsgObjPlayerInput,
        version: u64,
    ) {
        self.game.set_player_inp(player_id, inp, version, false)
    }
}
