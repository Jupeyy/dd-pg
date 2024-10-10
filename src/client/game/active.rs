use std::{
    collections::BTreeMap,
    net::SocketAddr,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use base::system::{System, SystemTimeInterface};
use client_console::console::remote_console::RemoteConsole;
use client_map::client_map::GameMap;
use client_render_game::render_game::{ObservedPlayer, RenderGameForPlayer};
use client_ui::connect::user_data::ConnectMode;
use demo::recorder::{DemoRecorder, DemoRecorderCreateProps};
use game_interface::{
    events::GameEvents,
    types::game::{GameEntityId, GameTickType},
};
use graphics::graphics::graphics::Graphics;
use graphics_backend::backend::GraphicsBackend;
use hashlink::LinkedHashMap;
use network::network::quinn_network::QuinnNetwork;
use pool::{
    datatypes::{PoolBTreeMap, PoolVec},
    mt_pool::Pool as MtPool,
    pool::Pool,
};
use shared_base::network::messages::{MsgClInputPlayerChain, PlayerInputChainable};
use shared_network::{
    game_event_generator::GameEventGenerator,
    messages::{ClientToServerMessage, GameMessage},
};
use sound::sound::SoundManager;

use crate::{
    client::components::network_logic::NetworkLogic,
    spatial_chat::spatial_chat::SpatialChatGameWorldTy,
};

use super::{data::GameData, DisconnectAutoCleanup};

pub struct ActiveGame {
    pub network_logic: NetworkLogic,
    pub network: QuinnNetwork,
    pub game_event_generator_client: Arc<GameEventGenerator>,
    pub has_new_events_client: Arc<AtomicBool>,

    pub map: GameMap,
    pub demo_recorder: Option<DemoRecorder>,

    pub demo_recorder_props: DemoRecorderCreateProps,

    pub game_data: GameData,

    pub events: PoolBTreeMap<GameTickType, (GameEvents, bool)>,

    pub map_votes_loaded: bool,

    pub render_players_pool: Pool<LinkedHashMap<GameEntityId, RenderGameForPlayer>>,
    pub render_observers_pool: Pool<Vec<ObservedPlayer>>,

    pub player_inputs_pool: Pool<LinkedHashMap<GameEntityId, PoolVec<PlayerInputChainable>>>,
    pub player_inputs_chainable_pool: Pool<Vec<PlayerInputChainable>>,
    pub player_inputs_chain_pool: MtPool<LinkedHashMap<GameEntityId, MsgClInputPlayerChain>>,
    pub player_inputs_chain_data_pool: MtPool<Vec<u8>>,
    pub player_inputs_ser_helper_pool: Pool<Vec<u8>>,
    pub events_pool: Pool<BTreeMap<GameTickType, (GameEvents, bool)>>,

    pub addr: SocketAddr,

    pub remote_console: RemoteConsole,
    pub rcon_secret: Option<[u8; 32]>,

    pub requested_account_details: bool,

    pub spatial_world: SpatialChatGameWorldTy,
    pub auto_cleanup: DisconnectAutoCleanup,
    pub connect_info: ConnectMode,

    pub graphics: Graphics,
    pub graphics_backend: Rc<GraphicsBackend>,
    pub sound: SoundManager,
    pub sys: System,
}

impl ActiveGame {
    pub fn send_input(
        &mut self,
        player_inputs: &LinkedHashMap<GameEntityId, PoolVec<PlayerInputChainable>>,
        sys: &dyn SystemTimeInterface,
    ) {
        if !player_inputs.is_empty() || !self.game_data.snap_acks.is_empty() {
            let mut player_inputs_send = self.player_inputs_chain_pool.new();
            for (player_id, player_inputs) in player_inputs.iter() {
                let player = self.game_data.local_players.get_mut(player_id).unwrap();
                let mut data = self.player_inputs_chain_data_pool.new();
                let (diff_id, def_inp) = player
                    .server_input
                    .as_ref()
                    .map(|inp| (Some(inp.id), inp.inp))
                    .unwrap_or_default();

                let mut def = self.player_inputs_ser_helper_pool.new();
                bincode::serde::encode_into_std_write(
                    def_inp,
                    &mut *def,
                    bincode::config::standard().with_fixed_int_encoding(),
                )
                .unwrap();

                let mut cur_diff = def;
                for player_input in player_inputs.iter() {
                    let mut inp = self.player_inputs_ser_helper_pool.new();
                    bincode::serde::encode_into_std_write(
                        player_input,
                        &mut *inp,
                        bincode::config::standard().with_fixed_int_encoding(),
                    )
                    .unwrap();

                    bin_patch::diff_exact_size(&cur_diff, &inp, &mut data).unwrap();

                    cur_diff = inp;
                }

                let player_input = player_inputs.last().unwrap();
                // this should be smaller than the number of inputs saved on the server
                let as_diff = if player.server_input_storage.len() < 10 {
                    player
                        .server_input_storage
                        .insert(self.game_data.input_id, *player_input);
                    true
                } else {
                    false
                };

                player_inputs_send.insert(
                    *player_id,
                    MsgClInputPlayerChain {
                        data,
                        diff_id,
                        as_diff,
                    },
                );
            }

            let cur_time = sys.time_get_nanoseconds();
            // remove some old sent input timings
            while self
                .game_data
                .sent_input_ids
                .first_key_value()
                .is_some_and(|(_, sent_at)| {
                    cur_time.saturating_sub(*sent_at) > Duration::from_secs(3)
                })
            {
                self.game_data.sent_input_ids.pop_first();
            }
            self.game_data
                .sent_input_ids
                .insert(self.game_data.input_id, cur_time);
            self.network
                .send_unordered_auto_to_server(&GameMessage::ClientToServer(
                    ClientToServerMessage::Inputs {
                        id: self.game_data.input_id,
                        inputs: player_inputs_send,
                        snap_ack: self.game_data.snap_acks.as_slice().into(),
                    },
                ));

            self.game_data.snap_acks.clear();
            self.game_data.input_id += 1;
        }
    }
}
