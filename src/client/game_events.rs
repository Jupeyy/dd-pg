use std::sync::Arc;

use base_io::io::IO;
use client_render_base::map::render_pipe::Camera;
use client_ui::connecting::user_data::ConnectModes;
use game_config::config::ConfigGame;
use graphics::graphics::graphics::Graphics;
use pool::datatypes::StringPool;
use shared_base::network::server_info::ServerInfo;
use shared_network::{
    game_event_generator::{GameEvents, GameEventsSigned},
    messages::GameMessage,
};

use base::system::System;
use config::config::ConfigEngine;
use network::network::event::NetworkEvent;
use ui_base::types::UIState;

use crate::game::Game;

pub struct GameEventPipeline<'a> {
    pub graphics: &'a mut Graphics,
    pub client: &'a mut Game,
    pub cam: &'a mut Camera,
    pub runtime_thread_pool: &'a mut Arc<rayon::ThreadPool>,
    pub io: &'a IO,
    pub config: &'a mut ConfigEngine,
    pub config_game: &'a mut ConfigGame,
    pub shared_info: &'a Arc<ServerInfo>,
    pub ui: &'a mut UIState,
    pub sys: &'a System,
    pub string_pool: &'a mut StringPool,
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
            Game::Active(game) => Some((
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
                let GameEventsSigned { msg: event, .. } = event;
                match event {
                    GameEvents::NetworkEvent(net_ev) => match net_ev {
                        NetworkEvent::Connected(_) => {
                            println!("connect time cl: {}", timestamp.as_nanos());
                        }
                        NetworkEvent::Disconnected(_reason) => {
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
