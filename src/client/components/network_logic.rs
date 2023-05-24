use std::time::Duration;

use arrayvec::ArrayString;

use math::math::vector::vec4_base;

use crate::{
    client::component::{
        ComponentComponent, ComponentGameMsg, ComponentLoadIOPipe, ComponentLoadPipe,
        ComponentLoadWhileIOPipe, ComponentLoadable, ComponentRenderable, ComponentUpdatable,
        ComponentUpdatePipe, GameMsgPipeline,
    },
    client_map::{ClientMap, ClientMapFile},
    network::messages::{
        ClientToServerMessage, ColorChannel, GameMessage, MsgClReady, MsgObjGameSkinPartInfo,
        MsgObjGameWeaponInfo, MsgObjPlayerInfo, NetworkStr, ServerToClientMessage,
    },
};

pub enum ClientConnectionState {
    NotConnected = 0,
    Connecting,
    Ready,
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
                    pipe.network.send_to_server(&GameMessage::ClientToServer(
                        ClientToServerMessage::Ready(MsgClReady {
                            player_info: MsgObjPlayerInfo {
                                name: NetworkStr::from("TODO").unwrap(),
                                clan: NetworkStr::from("TODO").unwrap(),
                                country: NetworkStr::from("GER").unwrap(),
                                skin_body: MsgObjGameSkinPartInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    color_swizzle_r: ColorChannel::R,
                                    color_swizzle_g: ColorChannel::G,
                                    color_swizzle_b: ColorChannel::B,
                                    color_swizzle_a: ColorChannel::A,
                                    color: vec4_base::<u8> {
                                        x: 255,
                                        y: 255,
                                        z: 255,
                                        w: 255,
                                    },
                                },
                                skin_ears: MsgObjGameSkinPartInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    color_swizzle_r: ColorChannel::R,
                                    color_swizzle_g: ColorChannel::G,
                                    color_swizzle_b: ColorChannel::B,
                                    color_swizzle_a: ColorChannel::A,
                                    color: vec4_base::<u8> {
                                        x: 255,
                                        y: 255,
                                        z: 255,
                                        w: 255,
                                    },
                                },
                                skin_feet: MsgObjGameSkinPartInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    color_swizzle_r: ColorChannel::R,
                                    color_swizzle_g: ColorChannel::G,
                                    color_swizzle_b: ColorChannel::B,
                                    color_swizzle_a: ColorChannel::A,
                                    color: vec4_base::<u8> {
                                        x: 255,
                                        y: 255,
                                        z: 255,
                                        w: 255,
                                    },
                                },
                                skin_hand: MsgObjGameSkinPartInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    color_swizzle_r: ColorChannel::R,
                                    color_swizzle_g: ColorChannel::G,
                                    color_swizzle_b: ColorChannel::B,
                                    color_swizzle_a: ColorChannel::A,
                                    color: vec4_base::<u8> {
                                        x: 255,
                                        y: 255,
                                        z: 255,
                                        w: 255,
                                    },
                                },
                                skin_decoration: MsgObjGameSkinPartInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    color_swizzle_r: ColorChannel::R,
                                    color_swizzle_g: ColorChannel::G,
                                    color_swizzle_b: ColorChannel::B,
                                    color_swizzle_a: ColorChannel::A,
                                    color: vec4_base::<u8> {
                                        x: 255,
                                        y: 255,
                                        z: 255,
                                        w: 255,
                                    },
                                },
                                skin_animation_name: NetworkStr::from("TODO").unwrap(),
                                skin_permanent_effect_name: NetworkStr::from("TODO").unwrap(),
                                skin_state_effects_name: NetworkStr::from("TODO").unwrap(),
                                skin_server_state_effects_name: NetworkStr::from("TODO").unwrap(),
                                skin_status_effects_name: NetworkStr::from("TODO").unwrap(),
                                pistol: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                                grenade: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                                laser: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                                puller: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                                shotgun: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                                hammer: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                                ninja: MsgObjGameWeaponInfo {
                                    name: NetworkStr::from("TODO").unwrap(),
                                    anim_name: NetworkStr::from("TODO").unwrap(),
                                    effect_name: NetworkStr::from("TODO").unwrap(),
                                },
                            },
                        }),
                    ));
                    self.cur_client_connection_state = ClientConnectionState::Ready;
                }
            }
            _ => {}
        }
    }
}

impl ComponentRenderable for NetworkLogic {}

impl ComponentGameMsg for NetworkLogic {
    fn on_msg(
        &mut self,
        timestamp: &Duration,
        msg: &ServerToClientMessage,
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
                    pipe.worker,
                    pipe.fs,
                    pipe.config,
                    &pipe.sys.time,
                ));
                println!("{}", info.map.as_str());
                let ping = *timestamp - pipe.client_data.server_connect_time;
                // set the first ping based on the intial packets,
                // later prefer the network stats
                pipe.client_data.ping = ping;
            }
            ServerToClientMessage::Snapshot(snap) => {
                pipe.snap_shot_builder
                    .convert_to_game_state(snap, &mut pipe.game);
                pipe.client_data.player_id_on_server = snap.recv_player_id;
                pipe.client_data.snapshot_timestamp = *timestamp;
            }
            _ => {}
        }
    }

    fn on_connect(&mut self, _timestamp: &Duration) {
        self.cur_client_connection_state = ClientConnectionState::Connecting;
    }

    fn on_disconnect(&mut self, _timestamp: &Duration) {
        self.cur_client_connection_state = ClientConnectionState::NotConnected;
    }
}

impl ComponentComponent for NetworkLogic {
    fn does_update(&self) -> bool {
        true
    }
    fn handles_msgs(&self) -> bool {
        true
    }
}

impl NetworkLogic {
    pub fn new() -> Self {
        Self {
            cur_map: String::new(),
            cur_client_connection_state: ClientConnectionState::NotConnected,
        }
    }
}
