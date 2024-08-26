use std::{collections::HashMap, net::IpAddr, sync::Arc, time::Duration};

use anyhow::anyhow;
use base::hash::{fmt_hash, name_and_hash, Hash};
use base_http::http_server::HttpDownloadServer;
use base_io::io::Io;
use base_io_traits::fs_traits::FileSystemWatcherItemInterface;
use game_database::traits::DbInterface;
use hashlink::LinkedHashMap;

use map::map::Map;
use network::network::connection::NetworkConnectionId;
use shared::game::state_wasm_manager::{GameStateMod, GameStateWasmManager, STATE_MODS_PATH};

use game_interface::{
    interface::{GameStateCreateOptions, GameStateInterface},
    rcon_commands::AuthLevel,
    types::{
        emoticons::EmoticonType,
        game::{GameEntityId, GameTickType},
        player_info::{PlayerClientInfo, PlayerDropReason},
        render::character::TeeEye,
    },
    votes::{VoteState, Voted},
};
use shared_base::{network::messages::GameModification, player_input::PlayerInput};

#[derive(Debug)]
pub struct ServerPlayer {
    pub network_id: NetworkConnectionId,
    pub id: GameEntityId,

    pub inp: PlayerInput,
}

impl ServerPlayer {
    pub fn new(network_id: &NetworkConnectionId, id: &GameEntityId) -> Self {
        Self {
            network_id: *network_id,
            id: *id,

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
        io: &Io,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<Self> {
        let map_file_str = map_name.to_string();
        let fs = io.fs.clone();
        let map = io.io_batcher.spawn(async move {
            let map_path = format!("map/maps/{}.twmap", map_file_str);
            let map_file = fs.read_file(map_path.as_ref()).await?;

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
                    .read_file(path.as_ref())
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
                    .read_file(path.as_ref())
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
            #[cfg(feature = "legacy")]
            Err(map_res_err) => {
                let map_path = format!("legacy/maps/{}.map", map_name);
                let map = map_convert_lib::legacy_to_new::legacy_to_new(
                    map_path.as_ref(),
                    &io.into(),
                    runtime_thread_pool,
                    true,
                )
                .map_err(|err| {
                    anyhow!(
                        "Loading map failed: {map_res_err}, legacy map loading failed too: {err}"
                    )
                })?;
                let mut map_bytes = Vec::new();
                map.map.write(&mut map_bytes, runtime_thread_pool)?;
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
            #[cfg(not(feature = "legacy"))]
            Err(err) => Err(err),
        }?;

        Ok(Self {
            name: map_name.to_string(),
            map_file,
            resource_files,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ClientAuth {
    pub cert: Arc<x509_cert::Certificate>,
    pub level: AuthLevel,
}

#[derive(Debug, Default)]
pub enum ServerExtraVoteInfo {
    Player {
        to_kick_player: NetworkConnectionId,
        ip: IpAddr,
        account_info: ClientAuth,
    },
    #[default]
    None,
}

#[derive(Debug)]
pub struct ServerVote {
    pub state: VoteState,
    pub started_at: Duration,

    pub extra_vote_info: ServerExtraVoteInfo,

    pub participating_clients: HashMap<NetworkConnectionId, Voted>,
}

pub const RESERVED_VANILLA_NAMES: [&str; 4] = ["", "vanilla", "native", "default"];
pub const RESERVED_DDNET_NAMES: [&str; 1] = ["ddnet"];

pub struct ServerGame {
    pub players: LinkedHashMap<GameEntityId, ServerPlayer>,
    pub game: GameStateWasmManager,
    pub cur_monotonic_tick: GameTickType,
    pub map: ServerMap,
    pub map_blake3_hash: Hash,
    pub game_mod: GameModification,

    game_mod_fs_change_watcher: Option<Box<dyn FileSystemWatcherItemInterface>>,

    pub http_server: Option<HttpDownloadServer>,

    // votes
    pub cur_vote: Option<ServerVote>,

    pub queued_inputs: LinkedHashMap<GameTickType, LinkedHashMap<GameEntityId, PlayerInput>>,
}

impl ServerGame {
    pub fn new(
        map_name: &str,
        game_mod: &str,
        config: Option<Vec<u8>>,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        io: &Io,
        db: &Arc<dyn DbInterface>,
    ) -> anyhow::Result<Self> {
        let map = ServerMap::new(map_name, io, runtime_thread_pool).unwrap();
        let (game_state_mod, game_mod, game_mod_file, game_mod_name, game_mod_blake3_hash) =
            match game_mod {
                x if RESERVED_VANILLA_NAMES.contains(&x) => (
                    GameStateMod::Native,
                    GameModification::Native,
                    Vec::new(),
                    "vanilla".to_string(),
                    None,
                ),
                x if RESERVED_DDNET_NAMES.contains(&x) => (
                    GameStateMod::Ddnet,
                    GameModification::Ddnet,
                    Vec::new(),
                    "ddnet".to_string(),
                    None,
                ),
                game_mod => {
                    let path = format!("{}/{}.wasm", STATE_MODS_PATH, game_mod);
                    let file_path = path.clone();
                    let (file, wasm_module) = {
                        let fs = io.fs.clone();

                        io.io_batcher
                            .spawn(async move {
                                let file = fs.read_file(file_path.as_ref()).await?;
                                let wasm_module =
                                    GameStateWasmManager::load_module(&fs, &file).await?;

                                Ok((file, wasm_module))
                            })
                            .get_storage()?
                    };
                    let (name, hash) = name_and_hash(game_mod, &file);
                    (
                        GameStateMod::Wasm { file: wasm_module },
                        GameModification::Wasm {
                            name: name.as_str().try_into()?,
                            hash,
                        },
                        file,
                        name,
                        Some(hash),
                    )
                }
            };
        let game = GameStateWasmManager::new(
            game_state_mod,
            map.map_file.clone(),
            map.name.clone(),
            GameStateCreateOptions {
                hint_max_characters: None, // TODO:
                config,
            },
            io,
            db.clone(),
        );
        let (map_name, map_hash) = name_and_hash(&map.name, &map.map_file);

        let fs_change_watcher = game_mod_blake3_hash.is_some().then(|| {
            // TODO: even tho watching individual files makes more sense, it should still make sure it's the same the server watches
            io.fs.watch_for_change(
                STATE_MODS_PATH.as_ref(),
                Some(format!("{}.wasm", game_mod_name).as_ref()),
            )
        });

        if let Some(config) = game.info.config.clone() {
            let game_mod_name = game_mod_name.clone();
            let fs = io.fs.clone();
            io.io_batcher.spawn_without_lifetime(async move {
                fs.create_dir("config".as_ref()).await?;
                fs.write_file(format!("config/{game_mod_name}.json").as_ref(), config)
                    .await?;
                Ok(())
            });
        }

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
            game_mod,

            game_mod_fs_change_watcher: fs_change_watcher,

            // votes
            cur_vote: None,

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
        network_id: &NetworkConnectionId,
        player_info: &PlayerClientInfo,
    ) -> GameEntityId {
        let player_id = self.game.player_join(player_info);
        self.players
            .insert(player_id, ServerPlayer::new(network_id, &player_id));
        player_id
    }

    pub fn player_drop(&mut self, player_id: &GameEntityId, reason: PlayerDropReason) {
        self.players.remove(player_id);
        self.game.player_drop(player_id, reason);
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
                && (for_monotonic_tick - cur_monotonic_tick) < self.game.game_tick_speed().get() * 3
            {
                let inp = self
                    .queued_inputs
                    .entry(for_monotonic_tick)
                    .or_insert_with(Default::default);
                let entry = inp.entry(*player_id).or_insert_with(Default::default);
                entry.try_overwrite(&player_input.inp, player_input.version(), false);
            }
        }
    }

    pub fn set_player_emoticon(&mut self, player_id: &GameEntityId, emoticon: EmoticonType) {
        self.game.set_player_emoticon(player_id, emoticon);
    }

    pub fn set_player_eye(&mut self, player_id: &GameEntityId, eye: TeeEye, duration: Duration) {
        self.game.set_player_eye(player_id, eye, duration)
    }
}
