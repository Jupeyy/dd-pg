pub mod snapshot {
    use shared_base::{
        game_types::TGameElementID, network::messages::MsgObjPlayerInfo,
        reuseable::CloneWithCopyableElements, types::GameTickType,
    };

    use crate::entities::{
        entity::entity::EntityInterface,
        projectile::projectile::{
            MtPoolProjectileReusableCore, ProjectileCore, ProjectileReusableCore,
        },
    };

    use super::super::{
        entities::character::character::{
            CharacterCore, CharacterReusableCore, MtPoolCharacterReusableCore,
        },
        player::player::{NoCharPlayer, NoCharPlayerType, Player, PlayerCharacterInfo},
        state::state::GameState,
    };
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use pool::{
        datatypes::PoolLinkedHashSet as SingleThreadedPoolLinkedHashSet,
        mt_datatypes::PoolLinkedHashMap, mt_pool::Pool,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotClientInfo {
        pub client_player_ids: SingleThreadedPoolLinkedHashSet<TGameElementID>,
        pub snap_everything: bool,
        pub snap_other_stages: bool,
        pub time_since_connect_nanos: u64,
    }

    #[derive(Debug, Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotCharacter {
        pub core: CharacterCore,
        pub reusable_core: MtPoolCharacterReusableCore,

        pub game_el_id: TGameElementID,
    }

    pub type PoolSnapshotCharacters = LinkedHashMap<TGameElementID, SnapshotCharacter>;
    pub type SnapshotCharacters = PoolLinkedHashMap<TGameElementID, SnapshotCharacter>;

    #[derive(Debug, Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotProjectile {
        pub core: ProjectileCore,
        pub reusable_core: MtPoolProjectileReusableCore,

        pub game_el_id: TGameElementID,
        pub owner_game_el_id: TGameElementID,
    }

    pub type PoolSnapshotProjectiles = LinkedHashMap<TGameElementID, SnapshotProjectile>;
    pub type SnapshotProjectiles = PoolLinkedHashMap<TGameElementID, SnapshotProjectile>;

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotWorld {
        pub characters: SnapshotCharacters,
        pub projectiles: SnapshotProjectiles,
    }

    impl SnapshotWorld {
        pub fn new(world_pool: &SnapshotWorldPool) -> Self {
            Self {
                characters: world_pool.characters_pool.new(),
                projectiles: world_pool.projectiles_pool.new(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotStage {
        pub world: SnapshotWorld,

        pub game_el_id: TGameElementID,
        pub stage_index: usize,
    }

    impl SnapshotStage {
        pub fn new(
            id: &TGameElementID,
            stage_index: usize,
            world_pool: &SnapshotWorldPool,
        ) -> Self {
            Self {
                world: SnapshotWorld::new(world_pool),
                game_el_id: id.clone(),
                stage_index,
            }
        }
    }

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotPlayer {
        pub game_el_id: TGameElementID,
        pub character_info: PlayerCharacterInfo,
    }

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotLocalPlayer {
        /// this information represents the values of the
        /// currently visible character the local player sees
        /// so e.g. if the local player is specing a different player
        /// these values are their values
        pub game_start_tick: GameTickType,
        pub animation_start_tick: GameTickType,
    }

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct SnapshotNoCharPlayer {
        pub game_el_id: TGameElementID,
        pub no_char_type: NoCharPlayerType,
    }

    pub struct SnapshotWorldPool {
        characters_pool: Pool<PoolSnapshotCharacters>,
        pub character_reusable_cores_pool: Pool<CharacterReusableCore>,
        projectiles_pool: Pool<PoolSnapshotProjectiles>,
        pub projectile_reusable_cores_pool: Pool<ProjectileReusableCore>,
    }

    impl SnapshotWorldPool {
        pub fn new(max_characters: usize) -> Self {
            Self {
                characters_pool: Pool::with_capacity(max_characters),
                // multiply by 2, because every character has two cores of this type
                character_reusable_cores_pool: Pool::with_capacity(max_characters * 2),
                projectiles_pool: Pool::with_capacity(1024), // TODO: no random number
                // multiply by 2, because every projectile has two cores of this type
                projectile_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: no random number
            }
        }
    }

    pub struct SnapshotPool {
        stages_pool: Pool<LinkedHashMap<TGameElementID, SnapshotStage>>,
        players_pool: Pool<LinkedHashMap<TGameElementID, SnapshotPlayer>>,
        no_char_players_pool: Pool<LinkedHashMap<TGameElementID, SnapshotNoCharPlayer>>,
        local_players_pool: Pool<LinkedHashMap<TGameElementID, SnapshotLocalPlayer>>,
    }

    impl SnapshotPool {
        pub fn new(max_characters: usize, max_local_players: usize) -> Self {
            Self {
                stages_pool: Pool::with_capacity(max_characters),
                players_pool: Pool::with_capacity(max_characters),
                no_char_players_pool: Pool::with_capacity(max_characters),
                local_players_pool: Pool::with_capacity(max_local_players),
            }
        }
    }

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct Snapshot {
        pub stages: PoolLinkedHashMap<TGameElementID, SnapshotStage>,
        pub players: PoolLinkedHashMap<TGameElementID, SnapshotPlayer>,
        pub no_char_players: PoolLinkedHashMap<TGameElementID, SnapshotNoCharPlayer>,
        pub game_tick: GameTickType,

        // the monotonic_tick is monotonic increasing
        // it's not related to the game tick and reflects
        // the ticks passed since the server started
        pub monotonic_tick: u64,

        pub local_players: PoolLinkedHashMap<TGameElementID, SnapshotLocalPlayer>,
        pub time_since_connect_nanos: u64,
    }

    impl Snapshot {
        pub fn new(
            game_tick: GameTickType,
            monotonic_tick: u64,
            time_since_connect_nanos: u64,
            pool: &SnapshotPool,
        ) -> Self {
            Self {
                stages: pool.stages_pool.new(),
                players: pool.players_pool.new(),
                no_char_players: pool.no_char_players_pool.new(),
                game_tick,
                monotonic_tick,
                local_players: pool.local_players_pool.new(),
                time_since_connect_nanos,
            }
        }
    }

    /// this is closely build like the type [`GameStateCreateOptions`]
    #[derive(Debug, Default)]
    pub struct SnapshotManagerCreateOptions {
        hint_max_characters: Option<usize>,
        hint_max_local_players: Option<usize>,
    }

    pub struct SnapshotManager {
        pub monotonic_tick: u64,

        // pools
        snapshot_pool: SnapshotPool,
        world_pool: SnapshotWorldPool,
    }

    impl SnapshotManager {
        pub fn new(options: &SnapshotManagerCreateOptions) -> Self {
            Self {
                monotonic_tick: 0,
                snapshot_pool: SnapshotPool::new(
                    options.hint_max_characters.unwrap_or(64),
                    options.hint_max_local_players.unwrap_or(4),
                ),
                world_pool: SnapshotWorldPool::new(options.hint_max_local_players.unwrap_or(64)),
            }
        }

        pub fn build_for(&self, game: &GameState, client: SnapshotClientInfo) -> Snapshot {
            let mut res = Snapshot::new(
                0,
                game.cur_monotonic_tick,
                client.time_since_connect_nanos,
                &self.snapshot_pool,
            );
            res.local_players.reserve(client.client_player_ids.len());
            client.client_player_ids.iter().for_each(|id| {
                if let Some(player) = game.players.get(id) {
                    res.local_players.insert(
                        id.clone(),
                        SnapshotLocalPlayer {
                            game_start_tick: player.game_start_tick,
                            animation_start_tick: player.animation_start_tick,
                        },
                    );
                } else if let Some(_) = game.no_char_players.get(id) {
                    res.local_players.insert(
                        id.clone(),
                        SnapshotLocalPlayer {
                            game_start_tick: GameTickType::default(),
                            animation_start_tick: GameTickType::default(),
                        },
                    );
                }
            });
            game.stages.values().for_each(|stage| {
                let mut characters = self.world_pool.characters_pool.new();
                stage.world.get_characters().iter().for_each(|(id, char)| {
                    let mut snap_char = SnapshotCharacter {
                        core: *char.get_core(),
                        reusable_core: self.world_pool.character_reusable_cores_pool.new(),
                        game_el_id: char.base.game_element_id.clone(),
                    };
                    snap_char
                        .reusable_core
                        .copy_clone_from(char.get_reusable_core());
                    characters.insert(id.clone(), snap_char);
                });
                let mut projectiles = self.world_pool.projectiles_pool.new();
                stage.world.get_projectiles().iter().for_each(|(id, proj)| {
                    let mut snap_proj = SnapshotProjectile {
                        core: *proj.projectile.get_core(),
                        reusable_core: self.world_pool.projectile_reusable_cores_pool.new(),
                        game_el_id: proj.projectile.base.game_element_id.clone(),
                        owner_game_el_id: proj.character_id.clone(),
                    };
                    snap_proj
                        .reusable_core
                        .copy_clone_from(proj.projectile.get_reusable_core());
                    projectiles.insert(id.clone(), snap_proj);
                });
                res.stages.insert(
                    stage.game_element_id.clone(),
                    SnapshotStage {
                        world: SnapshotWorld {
                            characters,
                            projectiles,
                        },
                        game_el_id: stage.game_element_id.clone(),
                        stage_index: stage.stage_index,
                    },
                );
            });
            game.players.values().for_each(|p| {
                res.players.insert(
                    p.id.clone(),
                    SnapshotPlayer {
                        game_el_id: p.id.clone(),
                        character_info: p.character_info.clone(),
                    },
                );
            });
            res
        }

        /**
         * Writes a snapshot into a game state
         * It uses a mutable reference to reuse vector capacity, heap objects etc.
         */
        #[must_use]
        pub fn convert_to_game_state(
            snapshot: &Snapshot,
            write_game_state: &mut GameState,
        ) -> bool {
            if snapshot.monotonic_tick <= write_game_state.snap_shot_manager.monotonic_tick {
                return false;
            }
            write_game_state.snap_shot_manager.monotonic_tick = snapshot.monotonic_tick;

            // drop all missing stages, we don't need the order here, since it will later be sorted anyway
            write_game_state.get_stages_mut().retain(|id, stage| {
                // every stage that is not in the snapshot must be removed
                if let Some(snap_stage) = snapshot.stages.get(&id) {
                    // same for characters
                    stage.world.get_characters_mut().retain(|char_id, _| {
                        if snap_stage.world.characters.contains_key(&char_id) {
                            true
                        } else {
                            false
                        }
                    });
                    // same for projectiles
                    stage.world.projectiles.retain(|proj_id, _| {
                        if snap_stage.world.projectiles.contains_key(&proj_id) {
                            true
                        } else {
                            false
                        }
                    });

                    true
                } else {
                    false
                }
            });

            // add new stages
            snapshot.stages.values().for_each(|stage| {
                // if the stage is new, add it to our list
                if !write_game_state
                    .get_stages()
                    .contains_key(&stage.game_el_id)
                {
                    write_game_state.insert_new_stage(stage.game_el_id.clone(), stage.stage_index);
                }

                // sorting by always moving the entry to the end (all entries will do this)
                write_game_state
                    .get_stages_mut()
                    .to_back(&stage.game_el_id)
                    .unwrap();

                // go through all characters of the stage, add missing ones
                stage.world.characters.values().for_each(|char| {
                    // if the character does not exist, add it
                    if !write_game_state.contains_character(&stage.game_el_id, &char.game_el_id) {
                        write_game_state.insert_new_character_to_stage(
                            &stage.game_el_id,
                            char.game_el_id.clone(),
                        );
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
                    let state_stage = write_game_state.get_stage_by_id_mut(&stage.game_el_id);
                    let stage_char = state_stage
                        .world
                        .get_characters_mut()
                        .to_back(&char.game_el_id)
                        .unwrap();
                    *stage_char.get_core_mut() = char.core;
                    stage_char
                        .get_reusable_core_mut()
                        .copy_clone_from(&char.reusable_core);
                });
                // go through all projectiles of the stage, add missing ones
                stage.world.projectiles.values().for_each(|proj| {
                    // if the projectile does not exist, add it
                    if !write_game_state.contains_projectile(&stage.game_el_id, &proj.game_el_id) {
                        write_game_state.insert_new_projectile_to_stage(
                            &stage.game_el_id,
                            proj.game_el_id.clone(),
                            proj.owner_game_el_id.clone(),
                            &proj.core.pos,
                            &proj.core.direction,
                            proj.core.life_span,
                            proj.core.damage,
                            proj.core.force,
                            proj.core.start_tick,
                            proj.core.is_explosive,
                        );
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
                    let state_stage = write_game_state.get_stage_by_id_mut(&stage.game_el_id);
                    let stage_proj = state_stage
                        .world
                        .projectiles
                        .to_back(&proj.game_el_id)
                        .unwrap();
                    *stage_proj.projectile.get_core_mut() = proj.core;
                    stage_proj
                        .projectile
                        .get_reusable_core_mut()
                        .copy_clone_from(&proj.reusable_core);
                });
            });

            snapshot.players.values().for_each(|p| {
                // check if the player is a player without char, then move it to players
                if let Some(no_char_player) = write_game_state.no_char_players.remove(&p.game_el_id)
                {
                    write_game_state.players.insert(
                        p.game_el_id.clone(),
                        Player::new(
                            no_char_player.player_info,
                            &no_char_player.id,
                            p.character_info.clone(),
                            no_char_player.version,
                            Default::default(),
                            Default::default(),
                        ),
                    );
                }
                // else check if the player is in the unknown players, move it
                else if let Some(ukn_player) =
                    write_game_state.unknown_players.remove(&p.game_el_id)
                {
                    write_game_state.players.insert(
                        p.game_el_id.clone(),
                        Player::new(
                            ukn_player.player_info,
                            &ukn_player.id,
                            p.character_info.clone(),
                            ukn_player.version,
                            Default::default(),
                            Default::default(),
                        ),
                    );
                }
                // in worst case we have a snapshot that came before the player info arrived, still insert it with default infos
                else if !write_game_state.players.contains_key(&p.game_el_id) {
                    write_game_state.players.insert(
                        p.game_el_id.clone(),
                        Player::new(
                            MsgObjPlayerInfo::explicit_default(),
                            &p.game_el_id,
                            p.character_info.clone(),
                            0,
                            Default::default(),
                            Default::default(),
                        ),
                    );
                }

                // sort
                write_game_state
                    .players
                    .to_back(&p.game_el_id)
                    .unwrap()
                    .character_info = p.character_info.clone();
            });
            // same with spec chars
            snapshot.no_char_players.values().for_each(|p| {
                // check if the player is a player, then move it to players without chars
                if let Some(no_char_player) = write_game_state.players.remove(&p.game_el_id) {
                    write_game_state.no_char_players.insert(
                        p.game_el_id.clone(),
                        NoCharPlayer::new(
                            &no_char_player.player_info,
                            &no_char_player.id,
                            no_char_player.version,
                            p.no_char_type,
                        ),
                    );
                }
                // else check if the player is in the unknown players, move it
                else if let Some(ukn_player) =
                    write_game_state.unknown_players.remove(&p.game_el_id)
                {
                    write_game_state.no_char_players.insert(
                        p.game_el_id.clone(),
                        NoCharPlayer::new(
                            &ukn_player.player_info,
                            &ukn_player.id,
                            ukn_player.version,
                            NoCharPlayerType::Unknown,
                        ),
                    );
                }
                // in worst case we have a snapshot that came before the player info arrived, still insert it with default infos
                else if !write_game_state.no_char_players.contains_key(&p.game_el_id) {
                    write_game_state.no_char_players.insert(
                        p.game_el_id.clone(),
                        NoCharPlayer::new(
                            &MsgObjPlayerInfo::explicit_default(),
                            &p.game_el_id,
                            0,
                            NoCharPlayerType::Unknown,
                        ),
                    );
                }

                // sort
                write_game_state.no_char_players.to_back(&p.game_el_id);
            });

            // drop players that are not in the snapshot
            write_game_state.players.retain_with_order(|id, _| {
                if snapshot.players.contains_key(&id) {
                    true
                } else {
                    false
                }
            });
            // same for no char players
            write_game_state.no_char_players.retain_with_order(|id, _| {
                if snapshot.no_char_players.contains_key(&id) {
                    true
                } else {
                    false
                }
            });

            write_game_state.cur_monotonic_tick = snapshot.monotonic_tick;

            true
        }
    }
}
