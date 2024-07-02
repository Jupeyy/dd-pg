use std::sync::Arc;

use base_io::io::Io;
use client_render_base::map::render_pipe::Camera;
use client_types::console::ConsoleEntry;
use client_ui::connecting::user_data::ConnectModes;
use game_config::config::ConfigGame;
use graphics::graphics::graphics::Graphics;
use pool::datatypes::StringPool;
use shared_base::network::server_info::ServerInfo;
use shared_network::{game_event_generator::GameEvents, messages::GameMessage};

use base::system::System;
use config::config::ConfigEngine;
use network::network::event::NetworkEvent;
use ui_base::types::UiState;

use crate::game::Game;

pub struct GameEventPipeline<'a> {
    pub graphics: &'a mut Graphics,
    pub client: &'a mut Game,
    pub cam: &'a mut Camera,
    pub runtime_thread_pool: &'a mut Arc<rayon::ThreadPool>,
    pub io: &'a Io,
    pub config: &'a mut ConfigEngine,
    pub config_game: &'a mut ConfigGame,
    pub shared_info: &'a Arc<ServerInfo>,
    pub ui: &'a mut UiState,
    pub sys: &'a System,
    pub string_pool: &'a mut StringPool,
    pub console_entries: &'a Vec<ConsoleEntry>,
}

pub struct GameEventsClient {}

impl GameEventsClient {
    pub fn update<'a, 'b>(pipe: &mut GameEventPipeline<'a>) {
        let event_gen = match pipe.client {
            Game::None | Game::PrepareConnect(_) => None,
            Game::Connecting(game) => Some((
                &game.has_new_events_client,
                &game.game_event_generator_client,
            )),
            Game::Loading(game) => Some((
                &game.has_new_events_client,
                &game.game_event_generator_client,
            )),
            Game::Active(game) | Game::WaitingForFirstSnapshot(game) => Some((
                &game.has_new_events_client,
                &game.game_event_generator_client,
            )),
        };

        if event_gen
            .as_ref()
            .is_some_and(|(has_events, _)| has_events.load(std::sync::atomic::Ordering::Relaxed))
        {
            let (has_events, events) = event_gen.unwrap();
            let mut events_guard = events.events.blocking_lock();
            has_events.store(false, std::sync::atomic::Ordering::Relaxed);
            let events = std::mem::take(&mut *events_guard);
            drop(events_guard);

            for (_, timestamp, event) in events {
                match event {
                    GameEvents::NetworkEvent(net_ev) => match net_ev {
                        NetworkEvent::Connected { .. } => {
                            println!("connect time cl: {}", timestamp.as_nanos());
                        }
                        NetworkEvent::Disconnected { .. } => {
                            pipe.config.ui.path.route("");
                            pipe.ui.is_ui_open = true;
                            *pipe.client = Game::None;
                        }
                        NetworkEvent::NetworkStats(stats) => {
                            if let Game::Active(game) = pipe.client {
                                game.client_data.ping = stats.ping;
                                let predict_timing = &mut game.client_data.prediction_timing;
                                predict_timing.add_ping(stats.ping, timestamp);
                            }
                        }
                        NetworkEvent::ConnectingFailed(reason) => {
                            if let Game::Connecting(game) = pipe.client {
                                // TODO:
                                game.connect_info.set(ConnectModes::Err { msg: reason });
                            }
                            pipe.config.ui.path.route("connecting");
                        }
                    },
                    GameEvents::NetworkMsg(game_msg) => {
                        match game_msg {
                            GameMessage::ServerToClient(server_to_client_msg) => {
                                pipe.client.on_msg(
                                    timestamp,
                                    server_to_client_msg,
                                    pipe.sys,
                                    pipe.runtime_thread_pool,
                                    pipe.io,
                                    pipe.ui,
                                    pipe.config,
                                    pipe.config_game,
                                    pipe.shared_info,
                                    pipe.string_pool,
                                    pipe.console_entries,
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
    }
}
