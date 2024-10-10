use std::{collections::BTreeMap, sync::Arc};

use base::system::System;
use base_io::io::Io;
use client_console::console::remote_console::RemoteConsole;
use client_map::client_map::GameMap;
use client_types::console::ConsoleEntry;
use client_ui::ingame_menu::account_info::AccountInfo;
use config::config::ConfigEngine;

use demo::recorder::DemoRecorder;
use game_config::config::ConfigGame;
use game_interface::{events::GameEvents, types::game::GameTickType};
use network::network::quinn_network::QuinnNetwork;
use pool::datatypes::StringPool;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::UiState;

use super::{
    game::data::GameData,
    spatial_chat::spatial_chat::{SpatialChat, SpatialChatGameWorld},
};

pub struct GameMsgPipeline<'a> {
    pub demo_recorder: &'a mut Option<DemoRecorder>,
    pub network: &'a mut QuinnNetwork,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub io: &'a Io,
    pub map: &'a mut GameMap,
    pub game_data: &'a mut GameData,
    pub console_entries: &'a Vec<ConsoleEntry>,
    pub events: &'a mut BTreeMap<GameTickType, (GameEvents, bool)>,
    pub config: &'a mut ConfigEngine,
    pub config_game: &'a mut ConfigGame,
    pub shared_info: &'a Arc<ServerInfo>,
    pub account_info: &'a AccountInfo,
    pub ui: &'a mut UiState,
    pub sys: &'a System,
    pub string_pool: &'a StringPool,
    pub remote_console: &'a mut RemoteConsole,
    pub spatial_world: Option<&'a mut SpatialChatGameWorld>,
    pub spatial_chat: &'a SpatialChat,
}
