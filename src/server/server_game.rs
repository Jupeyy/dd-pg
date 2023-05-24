use std::{ops::ControlFlow, sync::Arc};

use arrayvec::ArrayString;
use network::network::network::NetworkConnectionID;

use crate::{
    datafile::{CDatafileWrapper, MapFileLayersReadOptions, MapFileOpenOptions},
    game::{
        collision::Collision,
        simulation_pipe::{LocalPlayerInput, SimulationPlayerInput},
        snapshot::SnapshotManager,
        state::GameState,
        GameElementGenerator, TGameElementID,
    },
    hash_queue::HashQueue,
    id_gen::{IDGenerator, IDGeneratorIDType},
    mapdef::{EEntityTiles, MapItemTypes},
    network::messages::{MsgObjPlayerInfo, MsgObjPlayerInput},
};

use base::system::SystemTimeInterface;

pub struct ServerPlayer {
    pub network_id: NetworkConnectionID,
    pub player_info: MsgObjPlayerInfo,
    pub input: MsgObjPlayerInput,
    pub local_input: LocalPlayerInput,
}

impl ServerPlayer {
    pub fn new(network_id: &NetworkConnectionID, player_info: &MsgObjPlayerInfo) -> Self {
        Self {
            network_id: *network_id,
            player_info: player_info.clone(),
            input: MsgObjPlayerInput::default(),
            local_input: Default::default(),
        }
    }
}

pub struct ServerMap {
    pub raw: CDatafileWrapper,
    pub collision: Collision,
}

impl ServerMap {
    pub fn new(
        map_file: &str,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
    ) -> Result<Self, ArrayString<4096>> {
        let map_file_name = map_file.to_string() + &".map";
        let mapfile = std::fs::read("data/".to_string() + map_file_name.as_str());
        if let Ok(map_) = mapfile {
            let mut file_wrap = CDatafileWrapper::new();
            let mut load_options = MapFileOpenOptions::default();
            load_options
                .dont_load_map_item
                .iter_mut()
                .for_each(|i| *i = true);
            load_options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_GROUP as usize] = false;
            load_options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_LAYER as usize] = false;
            let res = file_wrap.Open(
                &map_,
                &map_file,
                runtime_thread_pool.as_ref(),
                &load_options,
                &sys,
            );
            if let Ok(data_start) = res {
                CDatafileWrapper::read_map_layers(
                    &file_wrap.data_file,
                    &mut file_wrap.layers,
                    data_start,
                    &sys,
                    &MapFileLayersReadOptions {
                        dont_load_design_layers: true,
                        ..Default::default()
                    },
                );
            }
            file_wrap.InitLayers(&runtime_thread_pool);

            let collision: Collision;
            let game_layer = file_wrap.get_game_layer();
            let w = game_layer.0.width as u32;
            let h = game_layer.0.height as u32;

            let tiles = game_layer.2.as_slice();
            collision = Collision::new(w, h, tiles);

            return Ok(Self {
                raw: file_wrap,
                collision,
            });
        }
        Err(ArrayString::from("Map could not be loaded.").unwrap())
    }
}

pub type ServerPlayerID = IDGeneratorIDType;

pub struct ServerGame {
    pub player_id_gen: IDGenerator,
    pub players: HashQueue<ServerPlayerID, ServerPlayer>,
    pub game: GameState,
    pub map: ServerMap,
    pub game_el_gen: GameElementGenerator,
    pub snap_shot_builder: SnapshotManager,
}

pub struct ServerGamePlayerInputForPipe<'a> {
    pub players: &'a HashQueue<ServerPlayerID, ServerPlayer>,
}

impl<'a> SimulationPlayerInput for ServerGamePlayerInputForPipe<'a> {
    fn get_input(&self, player_id: TGameElementID) -> Option<&LocalPlayerInput> {
        let player = self.players.get(&player_id);
        match player {
            Some(p) => Some(&p.local_input),
            None => None,
        }
    }
}

impl ServerGame {
    pub fn new(
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
        start_map: &str,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> Self {
        Self {
            player_id_gen: IDGenerator::new(),
            players: HashQueue::new(),
            game: GameState::new(),
            map: ServerMap::new(start_map, runtime_thread_pool, sys).unwrap(),
            game_el_gen: GameElementGenerator::default(),
            snap_shot_builder: SnapshotManager::new(),
        }
    }

    pub fn player_join(
        &mut self,
        network_id: &NetworkConnectionID,
        player_info: &MsgObjPlayerInfo,
    ) -> ServerPlayerID {
        let player_id = self.player_id_gen.get_next();
        self.players
            .add_or_set(player_id, ServerPlayer::new(network_id, player_info));

        // spawn and send character info
        let stage_index = self.game.add_stage(&mut self.game_el_gen);
        let char = self
            .game
            .get_stage_mut(stage_index)
            .get_world_mut()
            .add_character(&mut self.game_el_gen, &player_id);

        let w = self.map.raw.get_game_layer().0.width;
        self.map
            .raw
            .get_game_layer()
            .2
            .iter()
            .enumerate()
            .try_for_each(|(index, tile)| {
                if tile.index == EEntityTiles::ENTITY_SPAWN as u8 {
                    let x = index % w as usize;
                    let y = index / w as usize;

                    char.cores[0].core.pos.x = x as f32 * 32.0 + 1.0;
                    char.cores[0].core.pos.y = y as f32 * 32.0 + 1.0;
                    return ControlFlow::Break(());
                }
                ControlFlow::Continue(())
            });

        player_id
    }

    pub fn player_inp(&mut self, player_id: &ServerPlayerID, inp: &MsgObjPlayerInput) {
        let player = self.players.get_mut(player_id).unwrap();
        player.input = *inp;
        player.local_input = LocalPlayerInput::from_net_obj(&inp)
    }
}
