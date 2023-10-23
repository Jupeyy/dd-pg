use std::sync::{atomic::AtomicBool, Arc};

use base_io::io::IO;
use client_containers::skins::SkinContainer;
use client_render_base::map::render_pipe::Camera;
use graphics_backend::types::Graphics;
use pool::datatypes::StringPool;
use shared_network::{
    game_event_generator::{GameEventGenerator, GameEvents},
    messages::GameMessage,
};

use base::system::System;
use config::config::Config;
use network::network::{network::NetworkGameEvent, quinn_network::QuinnNetwork};
use ui_base::types::UIState;

use super::{client::Client, component::GameMsgPipeline};

pub struct GameEventPipeline<'a> {
    pub event_generator: &'a GameEventGenerator,
    pub event_generator_has_events: &'a AtomicBool,
    pub network: &'a mut QuinnNetwork,
    pub graphics: &'a mut Graphics,
    pub client: &'a mut Client,
    pub cam: &'a mut Camera,
    pub runtime_thread_pool: &'a mut Arc<rayon::ThreadPool>,
    pub io: &'a IO,
    pub config: &'a mut Config,
    pub ui: &'a mut UIState,
    pub sys: &'a System,
    pub skin_container: &'a mut SkinContainer,
    pub string_pool: &'a mut StringPool,
}

pub struct GameEventsClient {}

impl GameEventsClient {
    pub fn new() -> Self {
        GameEventsClient {}
    }

    pub fn update<'a, 'b>(&mut self, pipe: &mut GameEventPipeline<'a>) {
        if pipe
            .event_generator_has_events
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut events = pipe.event_generator.events.blocking_lock();
            for (con_id, timestamp_nanos, event) in events.drain(..) {
                match event {
                    GameEvents::NetworkEvent(net_ev) => match net_ev {
                        NetworkGameEvent::Connected => {
                            println!("connect time cl: {}", timestamp_nanos.as_nanos());
                            pipe.client.client_data.cur_server = con_id;
                            pipe.client.client_data.server_connect_time = timestamp_nanos;
                            pipe.client.network_logic.on_connect(&timestamp_nanos);
                        }
                        NetworkGameEvent::Disconnected(_reason) => {
                            pipe.client.network_logic.on_disconnect(&timestamp_nanos);
                            if con_id == pipe.client.client_data.cur_server {
                                pipe.client.client_data.cur_server = Default::default();
                            }
                        }
                        NetworkGameEvent::NetworkStats(stats) => {
                            pipe.client.client_data.ping = stats.ping;
                        }
                        NetworkGameEvent::ConnectingFailed(reason) => {
                            pipe.client.client_data.network_err = reason;
                            pipe.config.ui.path.route("connecterror");
                        }
                    },
                    GameEvents::NetworkMsg(game_msg) => {
                        if con_id == pipe.client.client_data.cur_server {
                            match game_msg {
                                GameMessage::ServerToClient(server_to_client_msg) => {
                                    pipe.client.network_logic.on_msg(
                                        &timestamp_nanos,
                                        server_to_client_msg,
                                        &mut GameMsgPipeline {
                                            network: pipe.network,
                                            graphics: pipe.graphics,
                                            runtime_thread_pool: pipe.runtime_thread_pool,
                                            io: pipe.io,
                                            map: &mut pipe.client.map,
                                            client_data: &mut pipe.client.client_data,
                                            config: pipe.config,
                                            ui: pipe.ui,
                                            sys: pipe.sys,
                                            skin_container: pipe.skin_container,
                                            cam: pipe.cam,
                                            string_pool: pipe.string_pool,
                                        },
                                    );
                                }
                                _ => {
                                    // ignore any client to server message
                                }
                            }
                        }
                    }
                }
            }
            pipe.event_generator_has_events
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
