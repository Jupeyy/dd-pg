use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use base::hash::{fmt_hash, name_and_hash, Hash};
use base_http::http_server::HttpDownloadServer;
use base_io::io::IO;
use base_io_traits::fs_traits::FileSystemWatcherItemInterface;
use hashlink::LinkedHashMap;

use map_convert_lib::legacy_to_new::legacy_to_new;

use map::map::Map;
use network::network::connection::NetworkConnectionID;
use shared::game::state_wasm_manager::{GameStateMod, GameStateWasmManager, STATE_MODS_PATH};

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
    pub name: String,
    pub map_file: Vec<u8>,
    pub resource_files: HashMap<String, Vec<u8>>,
}

impl ServerMap {
    pub fn new(
        map_name: &str,
        io: &IO,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<Self> {
        let map_file_str = map_name.to_string();
        let fs = io.fs.clone();
        let map = io.io_batcher.spawn(async move {
            let map_path = format!("map/maps/{}.twmap", map_file_str);
            let map_file = fs.open_file(map_path.as_ref()).await?;

            let (resources, _) = Map::read_resources_and_header(&map_file)?;
            let mut resource_files: HashMap<String, Vec<u8>> = Default::default();
            for image in resources.images.iter().chain(resources.image_arrays.iter()) {
                let path = format!(
                    "map/resources/images/{}_{}.{}",
                    image.name,
                    fmt_hash(&image.blake3_hash),
                    image.ty
                );
                let img_file = fs
                    .open_file(path.as_ref())
                    .await
                    .map_err(|err| anyhow!("loading images failed: {err}"))?;
                resource_files.insert(path, img_file);
            }

            for sound in &resources.sounds {
                let path = format!(
                    "map/resources/sounds/{}_{}.{}",
                    sound.name,
                    fmt_hash(&sound.blake3_hash),
                    sound.ty
                );
                let snd_file = fs
                    .open_file(path.as_ref())
                    .await
                    .map_err(|err| anyhow!("loading sound failed: {err}"))?;
                resource_files.insert(path, snd_file);
            }

            Ok((map_file, resource_files))
        });

        let map_res = map.get_storage();

        // try to load legacy map with that name, convert it to new format
        let (map_file, resource_files) = match map_res {
            Ok((map_file, resource_files)) => anyhow::Ok((map_file, resource_files)),
            Err(map_res_err) => {
                let map_path = format!("legacy/maps/{}.map", map_name);
                let map = legacy_to_new(map_path.as_ref(), &io.into(), &runtime_thread_pool, true)
                    .map_err(|err| {
                        anyhow!(
                            "Loading map failed: {map_res_err}, legacy map loading failed too: {err}"
                        )
                    })?;
                let mut map_bytes = Vec::new();
                map.map.write(&mut map_bytes, &runtime_thread_pool)?;
                let mut resource_files: HashMap<String, Vec<u8>> = Default::default();
                for resource in map.resources.images.into_iter() {
                    let path = format!(
                        "map/resources/images/{}_{}.{}",
                        resource.name,
                        fmt_hash(&resource.blake3_hash),
                        resource.ty
                    );
                    resource_files.insert(path, resource.buf);
                }
                for resource in map.resources.sounds.into_iter() {
                    let path = format!(
                        "map/resources/sounds/{}_{}.{}",
                        resource.name,
                        fmt_hash(&resource.blake3_hash),
                        resource.ty
                    );
                    resource_files.insert(path, resource.buf);
                }
                Ok((map_bytes, resource_files))
            }
        }?;

        Ok(Self {
            name: map_name.to_string(),
            map_file,
            resource_files,
        })
    }
}

pub struct ServerGame {
    pub players: LinkedHashMap<GameEntityId, ServerPlayer>,
    pub game: GameStateWasmManager,
    pub cur_monotonic_tick: GameTickType,
    pub map: ServerMap,
    pub map_blake3_hash: Hash,
    pub game_mod_name: String,
    pub game_mod_blake3_hash: Option<Hash>,

    game_mod_fs_change_watcher: Option<Box<dyn FileSystemWatcherItemInterface>>,

    pub http_server: Option<HttpDownloadServer>,

    pub queued_inputs: LinkedHashMap<GameTickType, LinkedHashMap<GameEntityId, PlayerInput>>,
}

impl ServerGame {
    pub fn new(
        map_name: &str,
        game_mod: &str,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        io: &IO,
    ) -> anyhow::Result<Self> {
        let map = ServerMap::new(map_name, io, runtime_thread_pool).unwrap();
        let (game_mod, game_mod_file, game_mod_name, game_mod_blake3_hash) = match game_mod {
            "" | "vanilla" | "native" => (GameStateMod::Native, Vec::new(), "".to_string(), None),
            "ddnet" => todo!(),
            game_mod => {
                let path = format!("{}/{}.wasm", STATE_MODS_PATH, game_mod);
                let file_path = path.clone();
                let file = {
                    let fs = io.fs.clone();

                    io.io_batcher
                        .spawn(async move { Ok(fs.open_file(file_path.as_ref()).await?) })
                        .get_storage()
                        .unwrap()
                };
                let (name, hash) = name_and_hash(game_mod, &file);
                (
                    GameStateMod::Wasm { file: file.clone() },
                    file,
                    name,
                    Some(hash),
                )
            }
        };
        let game =
            GameStateWasmManager::new(game_mod, map.map_file.clone(), Default::default(), io);
        let (map_name, map_hash) = name_and_hash(&map.name, &map.map_file);

        let fs_change_watcher = game_mod_blake3_hash.is_some().then(|| {
            // TODO: even tho watching individual files makes more sense, it should still make sure it's the same the server watches
            io.fs.watch_for_change(
                STATE_MODS_PATH.as_ref(),
                Some(format!("{}.wasm", game_mod_name).as_ref()),
            )
        });

        Ok(Self {
            http_server: {
                Some(HttpDownloadServer::new(
                    vec![(
                        format!("map/maps/{}_{}.twmap", map_name, fmt_hash(&map_hash)),
                        map.map_file.clone(),
                    )]
                    .into_iter()
                    .chain(map.resource_files.clone().into_iter())
                    .chain(
                        game_mod_blake3_hash
                            .map(|game_mod_blake3_hash| {
                                (
                                    format!(
                                        "{}/{}_{}.wasm",
                                        STATE_MODS_PATH,
                                        game_mod_name,
                                        fmt_hash(&game_mod_blake3_hash)
                                    ),
                                    game_mod_file,
                                )
                            })
                            .into_iter(),
                    )
                    .collect(),
                )?)
            },

            players: Default::default(),
            game,
            cur_monotonic_tick: 0,
            map,
            map_blake3_hash: map_hash,
            game_mod_name,
            game_mod_blake3_hash,

            game_mod_fs_change_watcher: fs_change_watcher,

            queued_inputs: Default::default(),
        })
    }

    pub fn should_reload(&self) -> bool {
        self.game_mod_fs_change_watcher
            .as_ref()
            .map(|watcher| watcher.has_file_change())
            .unwrap_or_default()
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
