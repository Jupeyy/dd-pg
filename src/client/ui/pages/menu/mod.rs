use std::sync::Arc;

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use client_render::map::client_map::ClientMap;
use config::config::Config;
use network::network::quinn_network::QuinnNetwork;
use shared_base::network::messages::{MsgClAddLocalPlayer, MsgObjPlayerInfo};
use shared_network::messages::{ClientToServerMessage, ClientToServerPlayerMessage, GameMessage};
use ui_base::types::UIFeedbackInterface;

use crate::client::client::ClientData;

pub mod connect_error;
pub mod connecting;
pub mod ingame;
pub mod main_menu;
pub mod queue;

pub struct ClientUIFeedback<'a> {
    network: &'a mut QuinnNetwork,
    fs: &'a Arc<FileSystem>,
    io_batcher: &'a TokIOBatcher,
    map: &'a mut ClientMap,
    client_data: &'a mut ClientData,
}

impl<'a> ClientUIFeedback<'a> {
    pub fn new(
        network: &'a mut QuinnNetwork,
        fs: &'a Arc<FileSystem>,
        io_batcher: &'a TokIOBatcher,
        map: &'a mut ClientMap,
        client_data: &'a mut ClientData,
    ) -> Self {
        Self {
            network,
            fs,
            io_batcher,
            map,
            client_data,
        }
    }
}

impl<'a> UIFeedbackInterface for ClientUIFeedback<'a> {
    fn network_connect(&mut self, addr: &str) {
        self.network.connect(addr);
    }

    fn network_connect_local_player(&mut self) {
        self.network.send_unordered_to(
            &GameMessage::ClientToServer(ClientToServerMessage::AddLocalPlayer(
                MsgClAddLocalPlayer {
                    player_info: MsgObjPlayerInfo::explicit_default(), // TODO
                },
            )),
            &self.network.get_current_connect_id(),
        )
    }

    fn network_disconnect_local_player(&mut self) {
        if self.client_data.local_players.len() > 1 {
            let (player_id, _) = self.client_data.local_players.pop_back().unwrap();
            self.network.send_unordered_to(
                &GameMessage::ClientToServer(ClientToServerMessage::PlayerMsg((
                    player_id,
                    ClientToServerPlayerMessage::RemLocalPlayer,
                ))),
                &self.network.get_current_connect_id(),
            )
        }
    }

    fn network_disconnect(&mut self) {
        self.network
            .disconnect(&self.network.get_current_connect_id());
        *self.map = ClientMap::None;
        *self.client_data = ClientData::default();
    }

    fn local_player_count(&self) -> usize {
        self.client_data.local_players.len()
    }

    fn queue_info(&self) -> &str {
        &self.client_data.queue_info
    }

    fn network_err(&self) -> &str {
        &self.client_data.network_err
    }

    fn call_path(&mut self, config: &mut Config, mod_name: &str, path: &str) {
        config.ui.path.try_route(mod_name, path)
    }
}
