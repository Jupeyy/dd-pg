use std::sync::Arc;

use base::system::System;
use base_io::io::IO;
use client_containers::skins::SkinContainer;
use client_map::client_map::ClientMap;
use client_render_base::map::render_pipe::Camera;
use config::config::ConfigEngine;

use game_config::config::ConfigGame;
use graphics::graphics::Graphics;
use network::network::quinn_network::QuinnNetwork;
use pool::datatypes::StringPool;
use ui_base::types::UIState;

use super::client::ClientData;

pub struct ComponentUpdatePipe<'a> {
    pub io: &'a IO,
    pub config: &'a ConfigGame,
    pub map: &'a ClientMap,
    pub network: &'a mut QuinnNetwork,
    pub sys: &'a mut System,
    pub client_data: &'a mut ClientData,
}

pub struct GameMsgPipeline<'a> {
    pub network: &'a mut QuinnNetwork,
    pub graphics: &'a mut Graphics,
    pub runtime_thread_pool: &'a mut Arc<rayon::ThreadPool>,
    pub io: &'a IO,
    pub map: &'a mut ClientMap,
    pub client_data: &'a mut ClientData,
    pub cam: &'a mut Camera,
    pub config: &'a mut ConfigEngine,
    pub ui: &'a mut UIState,
    pub sys: &'a System,
    pub skin_container: &'a mut SkinContainer,
    pub string_pool: &'a mut StringPool,
}
