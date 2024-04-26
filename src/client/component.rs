use std::sync::Arc;

use base::system::System;
use base_io::io::IO;
use client_demo::DemoRecorder;
use client_map::client_map::GameMap;
use config::config::ConfigEngine;

use game_config::config::ConfigGame;
use game_interface::events::GameEvents;
use network::network::quinn_network::QuinnNetwork;
use pool::datatypes::StringPool;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::UIState;

use crate::game::GameData;

pub struct GameMsgPipeline<'a> {
    pub demo_recorder: &'a mut Option<DemoRecorder>,
    pub network: &'a mut QuinnNetwork,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub io: &'a IO,
    pub map: &'a mut GameMap,
    pub client_data: &'a mut GameData,
    pub events: &'a mut GameEvents,
    pub config: &'a mut ConfigEngine,
    pub config_game: &'a mut ConfigGame,
    pub shared_info: &'a Arc<ServerInfo>,
    pub ui: &'a mut UIState,
    pub sys: &'a System,
    pub string_pool: &'a StringPool,
}
