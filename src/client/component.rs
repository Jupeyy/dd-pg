use std::{sync::Arc, time::Duration};

use arrayvec::ArrayString;

use base::system::System;
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;

use graphics_backend::types::Graphics;
use network::network::quinn_network::QuinnNetwork;
use shared_network::messages::ServerToClientMessage;
use ui_base::types::UIState;
use ui_wasm_manager::UIWinitWrapper;

use crate::{client_map::ClientMap, containers::skins::SkinContainer};

use super::{client::ClientData, render_pipe::Camera};

pub struct ComponentLoadPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub config: &'a Config,
}

pub struct ComponentLoadWhileIOPipe<'a> {
    pub config: &'a Config,
    pub runtime_threadpool: &'a Arc<rayon::ThreadPool>,
    pub sys: &'a System,
}

pub struct ComponentLoadIOPipe<'a> {
    pub fs: &'a Arc<FileSystem>,
    pub batcher: &'a TokIOBatcher,
    pub config: &'a Config,
}

pub trait ComponentLoadable {
    /**
     * The component should append all io related
     * tasks to the given io batcher, which it to initialize the component before use.
     * This together with init_while_io optimizes CPU workload with IO waiting for faster
     * startup times.
     * It's generally good style to move as much work as possible to the threadpool.
     */
    fn load_io(&mut self, io_pipe: &mut ComponentLoadIOPipe);
    /**
     * The component can use this function to initialize
     * memory heaps and other CPU/memory related stuff that can cleanly execute
     * while the io batcher executes
     */
    fn init_while_io(&mut self, pipe: &mut ComponentLoadWhileIOPipe);
    /**
     * At this point the component can rely on previously initialized
     * components.
     * For example it can now upload to the GPU
     * When the call to this component finishes it should be considered initialized
     * which means it should be able to be used fully in a sense of non blocking operations.
     * E.g. if it has to make file system calls, it needs to do them
     * in the fs thread pool then and cannot assume the file was already loaded.
     */
    fn init(&mut self, pipe: &mut ComponentLoadPipe) -> Result<(), ArrayString<4096>>;
}

pub struct ComponentUpdatePipe<'a> {
    pub fs: &'a Arc<FileSystem>,
    pub batcher: &'a TokIOBatcher,
    pub config: &'a Config,
    pub map: &'a ClientMap,
    pub network: &'a mut QuinnNetwork,
    pub sys: &'a mut System,
    pub client_data: &'a mut ClientData,
}

pub trait ComponentUpdatable {
    fn update(&mut self, _pipe: &mut ComponentUpdatePipe) {
        panic!("this function was not implemented");
    }
}

pub struct GameMsgPipeline<'a> {
    pub network: &'a mut QuinnNetwork,
    pub graphics: &'a mut Graphics,
    pub runtime_thread_pool: &'a mut Arc<rayon::ThreadPool>,
    pub io_batcher: &'a TokIOBatcher,
    pub fs: &'a Arc<FileSystem>,
    pub map: &'a mut ClientMap,
    pub client_data: &'a mut ClientData,
    pub cam: &'a mut Camera,
    pub config: &'a mut Config,
    pub ui: &'a mut UIState<UIWinitWrapper>,
    pub sys: &'a System,
    pub skin_container: &'a mut SkinContainer,
}

pub trait ComponentGameMsg {
    fn on_msg(
        &mut self,
        _timestamp: &Duration,
        _msg: ServerToClientMessage,
        _pipe: &mut GameMsgPipeline,
    ) {
        panic!("this function was not implemented");
    }

    // handle specific events seperately
    fn on_connect(&mut self, _timestamp: &Duration) {
        panic!("this function was not implemented");
    }
    fn on_disconnect(&mut self, _timestamp: &Duration) {
        panic!("this function was not implemented");
    }
}
