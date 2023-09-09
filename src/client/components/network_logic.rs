use std::time::Duration;

use arrayvec::ArrayString;

use client_render::map::client_map::{ClientMap, ClientMapFile};
use native::input::binds::BindKey;
use shared_base::binds::{BindActions, BindActionsLocalPlayer};
use shared_base::game_types::TGameElementID;
use shared_base::{
    game_types::time_until_tick,
    network::messages::{MsgClReady, MsgObjPlayerInfo},
};
use shared_game::state::state::GameStateInterface;
use shared_network::messages::{ClientToServerMessage, GameMessage, ServerToClientMessage};
use winit::{event::MouseButton, keyboard::KeyCode};

use crate::{
    client::component::{
        ComponentGameMsg, ComponentLoadIOPipe, ComponentLoadPipe, ComponentLoadWhileIOPipe,
        ComponentLoadable, ComponentUpdatable, ComponentUpdatePipe, GameMsgPipeline,
    },
    localplayer::ClientPlayer,
};

pub enum ClientConnectionState {
    NotConnected = 0,
    Connecting,
    Ingame,
}

/**
 * This component makes sure the client sends the
 * network events based on the current state on the server
 * E.g. if the map is about to connect
 * but requires to load the map,
 * it needs to send the "Ready" event as soon as the client
 * is ready.
 */
pub struct NetworkLogic {
    cur_map: String,
    cur_client_connection_state: ClientConnectionState,

    all_players_info_helper: Vec<(TGameElementID, MsgObjPlayerInfo)>,
}

impl ComponentLoadable for NetworkLogic {
    fn load_io(&mut self, _io_pipe: &mut ComponentLoadIOPipe) {}

    fn init_while_io(&mut self, _pipe: &mut ComponentLoadWhileIOPipe) {}

    fn init(&mut self, _pipe: &mut ComponentLoadPipe) -> Result<(), ArrayString<4096>> {
        Ok(())
    }
}

impl ComponentUpdatable for NetworkLogic {
    fn update(&mut self, pipe: &mut ComponentUpdatePipe) {
        match self.cur_client_connection_state {
            ClientConnectionState::Connecting => {
                // check if the client is ready
                // check if the map is loaded
                if pipe.map.is_fully_loaded() {
                    pipe.network
                        .send_unordered_to_server(&GameMessage::ClientToServer(
                            ClientToServerMessage::Ready(MsgClReady {
                                player_info: MsgObjPlayerInfo::explicit_default(),
                            }),
                        ));
                    self.cur_client_connection_state = ClientConnectionState::Ingame;
                }
            }
            _ => {}
        }
    }
}

impl ComponentGameMsg for NetworkLogic {
    fn on_msg(
        &mut self,
        timestamp: &Duration,
        msg: ServerToClientMessage,
        pipe: &mut GameMsgPipeline,
    ) {
        match msg {
            ServerToClientMessage::ServerInfo(info) => {
                self.cur_map = info.map.as_str().to_string();
                *pipe.map = ClientMap::UploadingImagesAndMapBuffer(ClientMapFile::new(
                    &pipe.runtime_thread_pool,
                    info.map.as_str(),
                    pipe.io_batcher,
                    pipe.graphics,
                    pipe.fs,
                    pipe.config,
                    &pipe.sys.time,
                ));
                println!("{}", info.map.as_str());
                let ping = *timestamp - pipe.client_data.server_connect_time;
                // set the first ping based on the intial packets,
                // later prefer the network stats
                pipe.client_data.ping = ping;
                pipe.cam.pos = info.hint_start_camera_pos;

                pipe.ui.is_ui_open = false;
                pipe.config.ui.path.route("ingame");
            }
            ServerToClientMessage::Snapshot {
                overhead_time,
                snapshot,
            } => {
                if let Some((_, game)) = pipe.map.try_get_data_and_game_mut() {
                    if game.convert_to_game_state(&snapshot) {
                        // set local players
                        pipe.client_data
                            .local_players
                            .retain_with_order(|player_id, _| {
                                if !snapshot.local_players.contains_key(player_id) {
                                    false
                                } else {
                                    true
                                }
                            });
                        snapshot
                            .local_players
                            .iter()
                            .for_each(|(id, _ /* TODO: */)| {
                                if !pipe.client_data.local_players.contains_key(&id) {
                                    let mut local_player: ClientPlayer = Default::default();
                                    let binds = &mut local_player.binds;
                                    binds.register_bind(
                                        &[BindKey::Key(KeyCode::KeyA)],
                                        BindActions::LocalPlayer(BindActionsLocalPlayer::MoveLeft),
                                    );
                                    binds.register_bind(
                                        &[BindKey::Key(KeyCode::KeyD)],
                                        BindActions::LocalPlayer(BindActionsLocalPlayer::MoveRight),
                                    );
                                    binds.register_bind(
                                        &[BindKey::Key(KeyCode::Space)],
                                        BindActions::LocalPlayer(BindActionsLocalPlayer::Jump),
                                    );
                                    binds.register_bind(
                                        &[BindKey::Key(KeyCode::Escape)],
                                        BindActions::LocalPlayer(BindActionsLocalPlayer::OpenMenu),
                                    );
                                    binds.register_bind(
                                        &[BindKey::Mouse(MouseButton::Left)],
                                        BindActions::LocalPlayer(BindActionsLocalPlayer::Fire),
                                    );
                                    binds.register_bind(
                                        &[BindKey::Mouse(MouseButton::Right)],
                                        BindActions::LocalPlayer(BindActionsLocalPlayer::Hook),
                                    );
                                    pipe.client_data
                                        .local_players
                                        .insert(id.clone(), local_player);
                                }
                                // sort
                                pipe.client_data.local_players.to_back(&id);
                            });

                        let monotonic_tick = game.cur_monotonic_tick();
                        // drop queued input that was before or at the server monotonic tick
                        while !pipe.client_data.input_per_tick.is_empty()
                            && *pipe.client_data.input_per_tick.front().unwrap().0
                                <= game.cur_monotonic_tick()
                        {
                            pipe.client_data.input_per_tick.pop_front();
                        }
                        let ticks_per_second = game.game_tick_speed();
                        let ping_nanos = pipe.client_data.ping.as_nanos() as u64;
                        let ticks_to_catch_up =
                            ping_nanos / time_until_tick(ticks_per_second).as_nanos() as u64;
                        let time_over_tick =
                            ping_nanos % time_until_tick(ticks_per_second).as_nanos() as u64;
                        (monotonic_tick + 1..(monotonic_tick + 1) + ticks_to_catch_up).for_each(
                            |new_tick| {
                                // apply the player input if the tick had any
                                if let Some(inp) = pipe.client_data.input_per_tick.get(&new_tick) {
                                    inp.iter().for_each(|(player_id, player_inp)| {
                                        game.set_player_inp(
                                            player_id,
                                            &player_inp.inp,
                                            player_inp.version,
                                            true,
                                        );
                                    });
                                }
                                game.tick();
                            },
                        );
                        // set the time until the next tick will happen client side
                        pipe.client_data.last_game_tick =
                            *timestamp - (Duration::from_nanos(time_over_tick) + overhead_time);

                        // update skin container to drop unused skins
                        // make sure that used skins are not dropped
                        game.all_players_info(&mut self.all_players_info_helper);
                        self.all_players_info_helper
                            .drain(..)
                            .for_each(|(_, info)| {
                                pipe.skin_container.get_or_default(
                                    info.skin_body.name.as_str(),
                                    pipe.graphics,
                                    pipe.fs,
                                    pipe.io_batcher,
                                    &pipe.runtime_thread_pool,
                                );
                            });
                        pipe.skin_container.update();
                    }
                }
            }
            ServerToClientMessage::PlayerInfo(player_info) => {
                if let Some((_, game)) = pipe.map.try_get_data_and_game_mut() {
                    game.try_overwrite_player_info(
                        &player_info.id,
                        &player_info.info,
                        player_info.version,
                    );
                }
            }
            ServerToClientMessage::PlayerInfos(mut player_infos) => {
                if let Some((_, game)) = pipe.map.try_get_data_and_game_mut() {
                    for player_info in player_infos.drain(..) {
                        game.try_overwrite_player_info(
                            &player_info.id,
                            &player_info.info,
                            player_info.version,
                        );
                    }
                }
            }
            ServerToClientMessage::Load(info) => {
                self.cur_map = info.map.as_str().to_string();
                *pipe.map = ClientMap::UploadingImagesAndMapBuffer(ClientMapFile::new(
                    &pipe.runtime_thread_pool,
                    info.map.as_str(),
                    pipe.io_batcher,
                    pipe.graphics,
                    pipe.fs,
                    pipe.config,
                    &pipe.sys.time,
                ));
                self.cur_client_connection_state = ClientConnectionState::Connecting;
            }
            ServerToClientMessage::QueueInfo(info) => {
                pipe.client_data.queue_info = info;
                pipe.config.ui.path.route("queue");
            }
        }
    }

    fn on_connect(&mut self, _timestamp: &Duration) {
        self.cur_client_connection_state = ClientConnectionState::Connecting;
    }

    fn on_disconnect(&mut self, _timestamp: &Duration) {
        self.cur_client_connection_state = ClientConnectionState::NotConnected;
    }
}

impl NetworkLogic {
    pub fn new() -> Self {
        Self {
            cur_map: String::new(),
            cur_client_connection_state: ClientConnectionState::NotConnected,

            all_players_info_helper: Default::default(),
        }
    }
}
