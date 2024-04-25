use std::sync::Arc;

use anyhow::anyhow;
use base::hash::{fmt_hash, Hash};
use base_http::http_server::HttpDownloadServer;
use base_io::io::IO;
use hashlink::LinkedHashMap;

use map_convert_lib::legacy_to_new::legacy_to_new;

use map::map::Map;
use network::network::connection::NetworkConnectionID;
use shared::game::state_wasm_manager::GameStateWasmManager;

use game_interface::{
    interface::GameStateInterface,
    types::{
        game::{GameEntityId, GameTickType},
        player_info::PlayerClientInfo,
    },
};
use shared_base::player_input::PlayerInput;

#[derive(Debug)]
pub struct ServerPlayer {
    pub network_id: NetworkConnectionID,
    pub id: GameEntityId,

    pub inp: PlayerInput,
}

impl ServerPlayer {
    pub fn new(network_id: &NetworkConnectionID, id: &GameEntityId) -> Self {
        Self {
            network_id: *network_id,
            id: id.clone(),

            inp: Default::default(),
        }
    }
}

pub struct ServerMap {
    pub map: Map,
    pub name: String,
    pub file: Vec<u8>,
    pub blake3_hash: Hash,
}

impl ServerMap {
    pub fn new(
        map_name: &str,
        io: &IO,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<Self> {
        let map_file_str = map_name.to_string();
        let fs = io.fs.clone();
        let tp = runtime_thread_pool.clone();
        let map = io.io_batcher.spawn(async move {
            let map_path = format!("map/maps/{}.twmap", map_file_str);
            let map_file = fs.open_file(map_path.as_ref()).await?;

            let map = Map::read(&map_file, &tp)?; // TODO: only read physics layer
            Ok((map, map_file))
        });

        let map_res = map.get_storage();

        // try to load legacy map with that name, convert it to new format
        let (map, map_file) = match map_res {
            Ok((map, map_file)) => anyhow::Ok((map, map_file)),
            Err(map_res_err) => {
                let map_path = format!("maps/{}.map", map_name);
                let map = legacy_to_new(map_path.as_ref(), &io.into(), &runtime_thread_pool, true)
                    .map_err(|err| {
                        anyhow!(
                            "Loading map failed: {map_res_err}, legacy map loading failed too: {err}"
                        )
                    })?;
                let mut map_bytes = Vec::new();
                map.map.write(&mut map_bytes, &runtime_thread_pool)?;
                Ok((map.map, map_bytes))
            }
        }?;

        let file_hash = Map::generate_hash_for(&map_file);

        Ok(Self {
            map: map,
            name: map_name.to_string(),
            file: map_file,
            blake3_hash: file_hash,
        })
    }
}

pub struct ServerGame {
    pub players: LinkedHashMap<GameEntityId, ServerPlayer>,
    pub game: GameStateWasmManager,
    pub cur_monotonic_tick: GameTickType,
    pub map: ServerMap,

    pub http_server: Option<HttpDownloadServer>,

    pub queued_inputs: LinkedHashMap<GameTickType, LinkedHashMap<GameEntityId, PlayerInput>>,
}

impl ServerGame {
    pub fn new(
        start_map: &str,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        io: &IO,
    ) -> anyhow::Result<Self> {
        let map = ServerMap::new(start_map, io, runtime_thread_pool).unwrap();
        let game = GameStateWasmManager::new(map.file.clone(), Default::default(), io);
        Ok(Self {
            http_server: Some(HttpDownloadServer::new(
                3000,
                vec![(
                    format!("map/maps/{}_{}.twmap", map.name, fmt_hash(&map.blake3_hash)),
                    map.file.clone(),
                )]
                .into_iter()
                .collect(),
            )?),

            players: Default::default(),
            game,
            cur_monotonic_tick: 0,
            map,

            queued_inputs: Default::default(),
        })
    }

    pub fn player_join(
        &mut self,
        network_id: &NetworkConnectionID,
        player_info: &PlayerClientInfo,
    ) -> GameEntityId {
        let player_id = self.game.player_join(player_info);
        self.players
            .insert(player_id.clone(), ServerPlayer::new(network_id, &player_id));
        player_id
    }

    pub fn player_drop(&mut self, player_id: &GameEntityId) {
        self.players.remove(player_id);
        self.game.player_drop(player_id);
    }

    pub fn player_inp(
        &mut self,
        player_id: &GameEntityId,
        player_input: PlayerInput,
        for_monotonic_tick: GameTickType,
    ) {
        if let Some(player) = self.players.get_mut(player_id) {
            let cur_monotonic_tick = self.cur_monotonic_tick;

            if for_monotonic_tick == cur_monotonic_tick + 1 {
                if let Some(diff) =
                    player
                        .inp
                        .try_overwrite(&player_input.inp, player_input.version(), false)
                {
                    self.game.set_player_input(player_id, &player.inp.inp, diff);
                }
            } else if for_monotonic_tick > cur_monotonic_tick + 1
                && (for_monotonic_tick - cur_monotonic_tick) < self.game.game_tick_speed() * 3
            {
                let inp = self
                    .queued_inputs
                    .entry(for_monotonic_tick)
                    .or_insert(Default::default());
                let entry = inp.entry(*player_id).or_insert(Default::default());
                entry.try_overwrite(&player_input.inp, player_input.version(), false);
            }
        }
    }
}
