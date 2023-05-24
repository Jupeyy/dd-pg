use std::sync::{atomic::AtomicBool, Arc};

use crate::{
    network::{
        game_event_generator::{GameEventGenerator, GameEvents},
        messages::GameMessage,
    },
    worker::Worker,
};

use base::{config::Config, filesys::FileSystem, io_batcher::IOBatcher, system::System};
use network::network::{network::NetworkGameEvent, quinn_network::QuinnNetwork};

use super::{client::Client, component::GameMsgPipeline};
use graphics::graphics::Graphics;
pub struct GameEventPipeline<'a, 'b> {
    pub event_generator: &'a tokio::sync::Mutex<GameEventGenerator>,
    pub event_generator_has_events: &'a AtomicBool,
    pub network: &'a mut QuinnNetwork,
    pub graphics: &'a mut Graphics,
    pub client: &'a mut Client<'b>,
    pub runtime_thread_pool: &'a mut Arc<rayon::ThreadPool>,
    pub io_batcher: &'a Arc<std::sync::Mutex<IOBatcher>>,
    pub worker: &'a mut Worker,
    pub fs: &'a Arc<FileSystem>,
    pub config: &'a Config,
    pub sys: &'a System,
}

pub struct GameEventsClient {}

impl GameEventsClient {
    pub fn new() -> Self {
        GameEventsClient {}
    }

    pub fn update<'a, 'b>(&mut self, pipe: &mut GameEventPipeline<'a, 'b>) -> bool {
        if pipe
            .event_generator_has_events
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut generator = pipe.event_generator.blocking_lock();
            for (con_id, timestamp_nanos, event) in &generator.events {
                match &event {
                    GameEvents::NetworkEvent(net_ev) => match net_ev {
                        NetworkGameEvent::Connected => {
                            println!("connect time cl: {}", timestamp_nanos.as_nanos());
                            pipe.client.client_data.cur_server = *con_id;
                            pipe.client.client_data.server_connect_time = *timestamp_nanos;
                            pipe.client
                                .components_that_handle_msgs
                                .iter()
                                .for_each(|index| {
                                    pipe.client.components[*index].on_connect(timestamp_nanos);
                                });
                        }
                        NetworkGameEvent::Disconnected(_reason) => {
                            pipe.client
                                .components_that_handle_msgs
                                .iter()
                                .for_each(|index| {
                                    pipe.client.components[*index].on_disconnect(timestamp_nanos);
                                });
                            if *con_id == pipe.client.client_data.cur_server {
                                pipe.client.client_data.cur_server = Default::default();
                            }
                        }
                        NetworkGameEvent::NetworkStats(_stats) => {
                            /*println!(
                                "ping: {}, inc latency: {}, out latency: {}",
                                stats.ping.unwrap_or_default().as_millis(),
                                stats.incoming_latency.unwrap_or_default().as_millis(),
                                stats.outgoing_latency.unwrap_or_default().as_millis()
                            );*/
                        }
                        _ => todo!(),
                    },
                    GameEvents::NetworkMsg(game_msg) => {
                        if *con_id == pipe.client.client_data.cur_server {
                            match game_msg {
                                GameMessage::ServerToClient(server_to_client_msg) => {
                                    pipe.client.components_that_handle_msgs.iter().for_each(
                                        |index| {
                                            pipe.client.components[*index].on_msg(
                                                timestamp_nanos,
                                                server_to_client_msg,
                                                &mut GameMsgPipeline {
                                                    network: pipe.network,
                                                    graphics: pipe.graphics,
                                                    runtime_thread_pool: pipe.runtime_thread_pool,
                                                    io_batcher: pipe.io_batcher,
                                                    worker: pipe.worker,
                                                    fs: pipe.fs,
                                                    map: &mut pipe.client.map,
                                                    game: &mut pipe.client.game,
                                                    snap_shot_builder: &mut pipe
                                                        .client
                                                        .snap_builder,
                                                    client_data: &mut pipe.client.client_data,
                                                    config: pipe.config,
                                                    sys: pipe.sys,
                                                },
                                            );
                                        },
                                    );
                                }
                                _ => {
                                    // ignore any client to server message
                                }
                            }
                        }
                    }
                    _ => todo!(),
                }
            }
            generator.events.clear();
            pipe.event_generator_has_events
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }

        true
    }
}
