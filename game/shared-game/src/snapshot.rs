pub mod snapshot {
    use std::{num::NonZeroU16, rc::Rc, sync::Arc};

    use base_log::log::SystemLog;
    use game_interface::types::{
        game::GameEntityId,
        id_gen::IdGenerator,
        snapshot::{SnapshotClientInfo, SnapshotLocalPlayer, SnapshotLocalPlayers},
        weapons::NUM_WEAPONS,
    };
    use hiarc::{hi_closure, Hiarc};
    use math::math::vector::vec2;
    use shared_base::reusable::CloneWithCopyableElements;

    use crate::{
        entities::{
            character::{
                character::CharacterPlayerTy,
                hook::character_hook::Hook,
                player::player::{
                    NoCharPlayer, NoCharPlayerType, NoCharPlayers, PlayerCharacterInfo, PlayerInfo,
                    Players, UknPlayers,
                },
            },
            entity::entity::EntityInterface,
            flag::flag::{FlagCore, FlagReusableCore, PoolFlagReusableCore},
            laser::laser::{LaserCore, LaserReusableCore, PoolLaserReusableCore},
            pickup::pickup::{PickupCore, PickupReusableCore, PoolPickupReusableCore},
            projectile::projectile::{
                PoolProjectileReusableCore, ProjectileCore, ProjectileReusableCore,
            },
        },
        game_objects::game_objects::GameObjectDefinitions,
        match_manager::match_manager::{MatchState, MatchType},
        stage::stage::Stages,
        types::types::GameOptions,
        world::world::{GameObjectWorld, WorldPool},
    };

    use super::super::{
        entities::character::character::{
            CharacterCore, CharacterReusableCore, PoolCharacterReusableCore,
        },
        state::state::GameState,
    };
    use hashlink::LinkedHashMap;
    use pool::{
        datatypes::{PoolLinkedHashMap, PoolVec},
        pool::Pool,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub enum SnapshotCharacterPlayerTy {
        None,
        Player,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotCharacter {
        pub core: CharacterCore,
        pub reusable_core: PoolCharacterReusableCore,
        pub ty: SnapshotCharacterPlayerTy,
        pub pos: vec2,
        pub hook: (Hook, Option<GameEntityId>),

        pub game_el_id: GameEntityId,
    }

    pub type PoolSnapshotCharacters = LinkedHashMap<GameEntityId, SnapshotCharacter>;
    pub type SnapshotCharacters = PoolLinkedHashMap<GameEntityId, SnapshotCharacter>;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotProjectile {
        pub core: ProjectileCore,
        pub reusable_core: PoolProjectileReusableCore,

        pub game_el_id: GameEntityId,
        pub owner_game_el_id: GameEntityId,
    }

    pub type PoolSnapshotProjectiles = LinkedHashMap<GameEntityId, SnapshotProjectile>;
    pub type SnapshotProjectiles = PoolLinkedHashMap<GameEntityId, SnapshotProjectile>;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotLaser {
        pub core: LaserCore,
        pub reusable_core: PoolLaserReusableCore,

        pub game_el_id: GameEntityId,
        pub owner_game_el_id: GameEntityId,
    }

    pub type PoolSnapshotLasers = LinkedHashMap<GameEntityId, SnapshotLaser>;
    pub type SnapshotLasers = PoolLinkedHashMap<GameEntityId, SnapshotLaser>;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotPickup {
        pub core: PickupCore,
        pub reusable_core: PoolPickupReusableCore,

        pub game_el_id: GameEntityId,
    }

    pub type PoolSnapshotPickups = LinkedHashMap<GameEntityId, SnapshotPickup>;
    pub type SnapshotPickups = PoolLinkedHashMap<GameEntityId, SnapshotPickup>;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotFlag {
        pub core: FlagCore,
        pub reusable_core: PoolFlagReusableCore,

        pub game_el_id: GameEntityId,
    }

    pub type PoolSnapshotFlags = LinkedHashMap<GameEntityId, SnapshotFlag>;
    pub type SnapshotFlags = PoolLinkedHashMap<GameEntityId, SnapshotFlag>;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotInactiveObject {
        pub hearts: PoolVec<GameObjectWorld>,
        pub shields: PoolVec<GameObjectWorld>,

        pub red_flags: PoolVec<GameObjectWorld>,
        pub blue_flags: PoolVec<GameObjectWorld>,

        pub weapons: [PoolVec<GameObjectWorld>; NUM_WEAPONS],

        pub ninjas: PoolVec<GameObjectWorld>,
    }

    pub type PoolSnapshotInactiveObjects = Vec<GameObjectWorld>;
    pub type SnapshotInactiveObjects = PoolVec<GameObjectWorld>;

    #[derive(Serialize, Deserialize)]
    pub struct SnapshotWorld {
        pub characters: SnapshotCharacters,
        pub projectiles: SnapshotProjectiles,
        pub lasers: SnapshotLasers,
        pub pickups: SnapshotPickups,
        pub flags: SnapshotFlags,

        pub inactive_objects: SnapshotInactiveObject,
    }

    impl SnapshotWorld {
        pub fn new(world_pool: &SnapshotWorldPool) -> Self {
            Self {
                characters: world_pool.characters_pool.new(),
                projectiles: world_pool.projectiles_pool.new(),
                lasers: world_pool.lasers_pool.new(),
                pickups: world_pool.pickups_pool.new(),
                flags: world_pool.flags_pool.new(),
                inactive_objects: SnapshotInactiveObject {
                    hearts: world_pool.inactive_objects.new(),
                    shields: world_pool.inactive_objects.new(),
                    red_flags: world_pool.inactive_objects.new(),
                    blue_flags: world_pool.inactive_objects.new(),
                    weapons: [
                        world_pool.inactive_objects.new(),
                        world_pool.inactive_objects.new(),
                        world_pool.inactive_objects.new(),
                        world_pool.inactive_objects.new(),
                        world_pool.inactive_objects.new(),
                    ],
                    ninjas: world_pool.inactive_objects.new(),
                },
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SnapshotMatchManager {
        ty: MatchType,
        state: MatchState,
    }

    impl SnapshotMatchManager {
        pub fn new(ty: MatchType, state: MatchState) -> Self {
            Self { ty, state }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct SnapshotStage {
        pub world: SnapshotWorld,
        pub match_manager: SnapshotMatchManager,

        pub game_el_id: GameEntityId,
        pub stage_index: usize,
    }

    #[derive(Serialize, Deserialize)]
    pub struct SnapshotPlayer {
        pub game_el_id: GameEntityId,
        pub character_info: PlayerCharacterInfo,
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub struct SnapshotNoCharPlayer {
        pub game_el_id: GameEntityId,
        pub no_char_type: NoCharPlayerType,
    }

    pub struct SnapshotWorldPool {
        characters_pool: Pool<PoolSnapshotCharacters>,
        pub character_reusable_cores_pool: Pool<CharacterReusableCore>,
        projectiles_pool: Pool<PoolSnapshotProjectiles>,
        pub projectile_reusable_cores_pool: Pool<ProjectileReusableCore>,
        lasers_pool: Pool<PoolSnapshotLasers>,
        pub laser_reusable_cores_pool: Pool<LaserReusableCore>,
        pickups_pool: Pool<PoolSnapshotPickups>,
        pub pickup_reusable_cores_pool: Pool<PickupReusableCore>,
        flags_pool: Pool<PoolSnapshotFlags>,
        pub flag_reusable_cores_pool: Pool<FlagReusableCore>,
        inactive_objects: Pool<PoolSnapshotInactiveObjects>,
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
                lasers_pool: Pool::with_capacity(1024), // TODO: no random number
                // multiply by 2, because every laser has two cores of this type
                laser_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: no random number
                pickups_pool: Pool::with_capacity(1024),                  // TODO: no random number
                // multiply by 2, because every pickup has two cores of this type
                pickup_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: no random number
                flags_pool: Pool::with_capacity(16),                       // TODO: no random number
                // multiply by 2, because every flag has two cores of this type
                flag_reusable_cores_pool: Pool::with_capacity(16 * 2), // TODO: no random number
                inactive_objects: Pool::with_capacity(16 * 2),         // TODO: no random number
            }
        }
    }

    pub struct SnapshotPool {
        pub(crate) stages_pool: Pool<LinkedHashMap<GameEntityId, SnapshotStage>>,
        no_char_players_pool: Pool<LinkedHashMap<GameEntityId, SnapshotNoCharPlayer>>,
        local_players_pool: Pool<LinkedHashMap<GameEntityId, SnapshotLocalPlayer>>,
    }

    impl SnapshotPool {
        pub fn new(max_characters: usize, max_local_players: usize) -> Self {
            Self {
                stages_pool: Pool::with_capacity(max_characters),
                no_char_players_pool: Pool::with_capacity(max_characters),
                local_players_pool: Pool::with_capacity(max_local_players),
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct Snapshot {
        pub stages: PoolLinkedHashMap<GameEntityId, SnapshotStage>,
        pub no_char_players: PoolLinkedHashMap<GameEntityId, SnapshotNoCharPlayer>,

        pub local_players: PoolLinkedHashMap<GameEntityId, SnapshotLocalPlayer>,
    }

    impl Snapshot {
        pub fn new(pool: &SnapshotPool) -> Self {
            Self {
                stages: pool.stages_pool.new(),
                no_char_players: pool.no_char_players_pool.new(),
                local_players: pool.local_players_pool.new(),
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
        // pools
        pub(crate) snapshot_pool: SnapshotPool,
        world_pool: SnapshotWorldPool,
    }

    impl SnapshotManager {
        pub fn new(options: &SnapshotManagerCreateOptions) -> Self {
            Self {
                snapshot_pool: SnapshotPool::new(
                    options.hint_max_characters.unwrap_or(64),
                    options.hint_max_local_players.unwrap_or(4),
                ),
                world_pool: SnapshotWorldPool::new(options.hint_max_local_players.unwrap_or(64)),
            }
        }

        pub(crate) fn build_stages(
            &self,
            stages: &mut PoolLinkedHashMap<GameEntityId, SnapshotStage>,
            game: &GameState,
        ) {
            game.stages.values().for_each(|stage| {
                let mut characters = self.world_pool.characters_pool.new();
                stage.world.characters.iter().for_each(|(id, char)| {
                    let mut snap_char = SnapshotCharacter {
                        core: *char.get_core(),
                        reusable_core: self.world_pool.character_reusable_cores_pool.new(),
                        pos: *char.pos.pos(),
                        hook: char.hook.get(),
                        game_el_id: char.base.game_element_id.clone(),
                        ty: if char.is_player_character() {
                            SnapshotCharacterPlayerTy::Player
                        } else {
                            SnapshotCharacterPlayerTy::None
                        },
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
                let mut lasers = self.world_pool.lasers_pool.new();
                stage.world.get_lasers().iter().for_each(|(id, laser)| {
                    let mut snap_laser = SnapshotLaser {
                        core: *laser.laser.get_core(),
                        reusable_core: self.world_pool.laser_reusable_cores_pool.new(),
                        game_el_id: laser.laser.base.game_element_id.clone(),
                        owner_game_el_id: laser.character_id.clone(),
                    };
                    snap_laser
                        .reusable_core
                        .copy_clone_from(laser.laser.get_reusable_core());
                    lasers.insert(id.clone(), snap_laser);
                });
                let mut pickups = self.world_pool.pickups_pool.new();
                stage.world.get_pickups().iter().for_each(|(id, pickup)| {
                    let mut snap_pickup = SnapshotPickup {
                        core: *pickup.get_core(),
                        reusable_core: self.world_pool.pickup_reusable_cores_pool.new(),
                        game_el_id: pickup.base.game_element_id.clone(),
                    };
                    snap_pickup
                        .reusable_core
                        .copy_clone_from(pickup.get_reusable_core());
                    pickups.insert(id.clone(), snap_pickup);
                });
                let mut flags = self.world_pool.flags_pool.new();
                stage.world.get_flags().iter().for_each(|(id, flag)| {
                    let mut snap_flag = SnapshotFlag {
                        core: *flag.get_core(),
                        reusable_core: self.world_pool.flag_reusable_cores_pool.new(),
                        game_el_id: flag.base.game_element_id.clone(),
                    };
                    snap_flag
                        .reusable_core
                        .copy_clone_from(flag.get_reusable_core());
                    flags.insert(id.clone(), snap_flag);
                });
                let add_inactive_obj =
                    |objs: &Vec<GameObjectWorld>, cont: &mut PoolSnapshotInactiveObjects| {
                        objs.iter().for_each(|obj| {
                            cont.push(*obj);
                        })
                    };

                let mut hearts = self.world_pool.inactive_objects.new();
                add_inactive_obj(
                    &stage.world.inactive_game_objects.pickups.hearts,
                    &mut hearts,
                );
                let mut shields = self.world_pool.inactive_objects.new();
                add_inactive_obj(
                    &stage.world.inactive_game_objects.pickups.shields,
                    &mut shields,
                );
                let mut red_flags = self.world_pool.inactive_objects.new();
                add_inactive_obj(
                    &stage.world.inactive_game_objects.pickups.red_flags,
                    &mut red_flags,
                );
                let mut blue_flags = self.world_pool.inactive_objects.new();
                add_inactive_obj(
                    &stage.world.inactive_game_objects.pickups.blue_flags,
                    &mut blue_flags,
                );
                let mut weapons = [
                    self.world_pool.inactive_objects.new(),
                    self.world_pool.inactive_objects.new(),
                    self.world_pool.inactive_objects.new(),
                    self.world_pool.inactive_objects.new(),
                    self.world_pool.inactive_objects.new(),
                ];
                for i in 0..NUM_WEAPONS {
                    add_inactive_obj(
                        &stage.world.inactive_game_objects.pickups.weapons[i],
                        &mut weapons[i],
                    );
                }
                let mut ninjas = self.world_pool.inactive_objects.new();
                add_inactive_obj(
                    &stage.world.inactive_game_objects.pickups.ninjas,
                    &mut ninjas,
                );

                stages.insert(
                    stage.game_element_id.clone(),
                    SnapshotStage {
                        world: SnapshotWorld {
                            characters,
                            projectiles,
                            lasers,
                            pickups,
                            flags,
                            inactive_objects: SnapshotInactiveObject {
                                hearts,
                                shields,
                                red_flags,
                                blue_flags,
                                weapons,
                                ninjas,
                            },
                        },
                        match_manager: SnapshotMatchManager::new(
                            stage.match_manager.game_match.ty,
                            stage.match_manager.game_match.state.clone(),
                        ),
                        game_el_id: stage.game_element_id.clone(),
                        stage_index: stage.stage_index,
                    },
                );
            });
        }

        pub fn snapshot_for(&self, game: &GameState, client: SnapshotClientInfo) -> Snapshot {
            let mut res = Snapshot::new(&self.snapshot_pool);
            res.local_players.reserve(client.client_player_ids.len());
            client.client_player_ids.iter().for_each(|id| {
                if let Some(p) = game
                    .players
                    .player(id)
                    .map(|p| {
                        game.stages
                            .get(&p.stage_id())
                            .map(|stage| stage.world.characters.get(id))
                            .flatten()
                    })
                    .flatten()
                {
                    res.local_players.insert(
                        id.clone(),
                        SnapshotLocalPlayer {
                            is_dummy: p.reusable_core.player_info.is_dummy,
                        },
                    );
                } else if let Some(p) = game.no_char_players.player(id) {
                    res.local_players.insert(
                        id.clone(),
                        SnapshotLocalPlayer {
                            is_dummy: p.player_info.is_dummy,
                        },
                    );
                }
            });
            self.build_stages(&mut res.stages, game);
            res
        }

        pub(crate) fn convert_to_game_stages(
            snap_stages: &PoolLinkedHashMap<GameEntityId, SnapshotStage>,
            stages: &mut Stages,
            world_pool: &WorldPool,
            game_object_definitions: &Rc<GameObjectDefinitions>,
            id_gen: &IdGenerator,
            game_options: &GameOptions,
            log: &Arc<SystemLog>,
            players: &Players,
            no_char_players: &NoCharPlayers,
            unknown_players: &mut UknPlayers,
            width: NonZeroU16,
            height: NonZeroU16,
        ) {
            // drop all missing stages, we don't need the order here, since it will later be sorted anyway
            stages.retain(|id, stage| {
                // every stage that is not in the snapshot must be removed
                if let Some(snap_stage) = snap_stages.get(&id) {
                    // same for characters
                    stage.world.characters.retain(|char_id, _| {
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
                    // same for lasers
                    stage.world.lasers.retain(|proj_id, _| {
                        if snap_stage.world.lasers.contains_key(&proj_id) {
                            true
                        } else {
                            false
                        }
                    });
                    // same for pickups
                    stage.world.pickups.retain(|proj_id, _| {
                        if snap_stage.world.pickups.contains_key(&proj_id) {
                            true
                        } else {
                            false
                        }
                    });
                    // same for flags
                    stage.world.flags.retain(|proj_id, _| {
                        if snap_stage.world.flags.contains_key(&proj_id) {
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

            // go through stages, add missing ones stages
            snap_stages.values().for_each(|snap_stage| {
                // if the stage is new, add it to our list
                if !stages.contains_key(&snap_stage.game_el_id) {
                    GameState::insert_new_stage(
                        stages,
                        snap_stage.game_el_id.clone(),
                        snap_stage.stage_index,
                        world_pool,
                        game_object_definitions,
                        width,
                        height,
                        id_gen,
                        *game_options,
                        log,
                    );
                }

                // sorting by always moving the entry to the end (all entries will do this)
                let state_stage = stages.to_back(&snap_stage.game_el_id).unwrap();

                let match_manager = &mut state_stage.match_manager;
                match_manager.game_match.ty = snap_stage.match_manager.ty;
                match_manager.game_match.state = snap_stage.match_manager.state.clone();

                // go through all characters of the stage, add missing ones
                snap_stage.world.characters.values().for_each(|char| {
                    // if the character does not exist, add it
                    if !state_stage.world.characters.contains_key(&char.game_el_id) {
                        let mut player_info = PlayerInfo::default();
                        let mut player_input = Default::default();
                        // check if the player is a player without char, then move it to players
                        if let Some(no_char_player) = no_char_players.remove(&char.game_el_id) {
                            // change character player info too
                            player_info = no_char_player.player_info;
                            player_input = no_char_player.player_input;
                        }
                        // else check if the player is in the unknown players, move it
                        else if let Some(ukn_player) = unknown_players.remove(&char.game_el_id) {
                            // change character player info too
                            player_info = ukn_player.player_info;
                        }

                        state_stage.world.add_character(
                            char.game_el_id.clone(),
                            &snap_stage.game_el_id,
                            player_info,
                            player_input,
                            &state_stage.match_manager.game_options,
                            match char.ty {
                                SnapshotCharacterPlayerTy::None => CharacterPlayerTy::None,
                                SnapshotCharacterPlayerTy::Player => CharacterPlayerTy::Player {
                                    players: players.clone(),
                                    no_char_players: no_char_players.clone(),
                                },
                            },
                            char.pos,
                        );

                        // sort
                        players.to_back(&char.game_el_id);
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
                    let stage_char = state_stage
                        .world
                        .characters
                        .to_back(&char.game_el_id)
                        .unwrap();
                    stage_char.update_player_ty(
                        &snap_stage.game_el_id,
                        match char.ty {
                            SnapshotCharacterPlayerTy::None => CharacterPlayerTy::None,
                            SnapshotCharacterPlayerTy::Player => CharacterPlayerTy::Player {
                                players: players.clone(),
                                no_char_players: no_char_players.clone(),
                            },
                        },
                    );
                    *stage_char.get_core_mut() = char.core;
                    stage_char
                        .get_reusable_core_mut()
                        .copy_clone_from(&char.reusable_core);
                    stage_char.pos.move_pos(char.pos);
                    stage_char.hook.set(char.hook.0, char.hook.1);
                });
                // go through all projectiles of the stage, add missing ones
                snap_stage.world.projectiles.values().for_each(|proj| {
                    // if the projectile does not exist, add it
                    if !state_stage.world.projectiles.contains_key(&proj.game_el_id) {
                        state_stage.world.insert_new_projectile(
                            proj.game_el_id.clone(),
                            proj.owner_game_el_id.clone(),
                            &proj.core.pos,
                            &proj.core.direction,
                            proj.core.life_span,
                            proj.core.damage,
                            proj.core.force,
                            proj.core.is_explosive,
                            proj.core.ty,
                        );
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
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
                // go through all lasers of the stage, add missing ones
                snap_stage.world.lasers.values().for_each(|proj| {
                    // if the laser does not exist, add it
                    if !state_stage.world.lasers.contains_key(&proj.game_el_id) {
                        state_stage.world.insert_new_laser(
                            proj.game_el_id.clone(),
                            proj.owner_game_el_id.clone(),
                            &proj.core.pos,
                            &proj.core.dir,
                            proj.core.energy,
                            proj.core.can_hit_others,
                            proj.core.can_hit_own,
                        );
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
                    let stage_proj = state_stage.world.lasers.to_back(&proj.game_el_id).unwrap();
                    *stage_proj.laser.get_core_mut() = proj.core;
                    stage_proj
                        .laser
                        .get_reusable_core_mut()
                        .copy_clone_from(&proj.reusable_core);
                });
                // go through all pickups of the stage, add missing ones
                snap_stage.world.pickups.values().for_each(|proj| {
                    // if the pickup does not exist, add it
                    if !state_stage.world.pickups.contains_key(&proj.game_el_id) {
                        state_stage.world.insert_new_pickup(
                            proj.game_el_id.clone(),
                            &proj.core.pos,
                            proj.core.ty,
                        );
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
                    let stage_proj = state_stage.world.pickups.to_back(&proj.game_el_id).unwrap();
                    *stage_proj.get_core_mut() = proj.core;
                    stage_proj
                        .get_reusable_core_mut()
                        .copy_clone_from(&proj.reusable_core);
                });
                // go through all flags of the stage, add missing ones
                snap_stage.world.flags.values().for_each(|flag| {
                    // if the flag does not exist, add it
                    if !state_stage.world.flags.contains_key(&flag.game_el_id) {
                        state_stage.world.insert_new_flag(
                            flag.game_el_id.clone(),
                            &flag.core.pos,
                            flag.core.ty,
                        );
                    }

                    // sorting by always moving the entry to the end (all entries will do this)
                    let stage_proj = state_stage.world.flags.to_back(&flag.game_el_id).unwrap();
                    *stage_proj.get_core_mut() = flag.core;
                    stage_proj
                        .get_reusable_core_mut()
                        .copy_clone_from(&flag.reusable_core);
                });

                state_stage
                    .world
                    .inactive_game_objects
                    .pickups
                    .hearts
                    .clone_from(&snap_stage.world.inactive_objects.hearts);
                state_stage
                    .world
                    .inactive_game_objects
                    .pickups
                    .shields
                    .clone_from(&snap_stage.world.inactive_objects.shields);
                state_stage
                    .world
                    .inactive_game_objects
                    .pickups
                    .red_flags
                    .clone_from(&snap_stage.world.inactive_objects.red_flags);
                state_stage
                    .world
                    .inactive_game_objects
                    .pickups
                    .blue_flags
                    .clone_from(&snap_stage.world.inactive_objects.blue_flags);
                state_stage
                    .world
                    .inactive_game_objects
                    .pickups
                    .weapons
                    .iter_mut()
                    .enumerate()
                    .for_each(|(index, weapon)| {
                        weapon.clone_from(&snap_stage.world.inactive_objects.weapons[index])
                    });
                state_stage
                    .world
                    .inactive_game_objects
                    .pickups
                    .ninjas
                    .clone_from(&snap_stage.world.inactive_objects.ninjas);
            });
        }

        /// Writes a snapshot into a game state.
        /// It uses a mutable reference to reuse vector capacity, heap objects etc.
        #[must_use]
        pub fn build_from_snapshot(
            snapshot: Snapshot,
            write_game_state: &mut GameState,
        ) -> SnapshotLocalPlayers {
            snapshot.no_char_players.values().for_each(|p| {
                // check if the player exists, then move it to players without chars
                if let Some(player) = write_game_state.players.player(&p.game_el_id) {
                    let stage = write_game_state.stages.get_mut(&player.stage_id()).unwrap();
                    stage
                        .world
                        .characters
                        .get_mut(&p.game_el_id)
                        .unwrap()
                        .move_to_spec_silent();
                    stage.world.characters.remove(&p.game_el_id);
                }
                // else check if the player is in the unknown players, move it
                else if let Some(ukn_player) =
                    write_game_state.unknown_players.remove(&p.game_el_id)
                {
                    write_game_state.no_char_players.insert(
                        p.game_el_id.clone(),
                        NoCharPlayer::new(
                            ukn_player.player_info,
                            Default::default(),
                            &ukn_player.id,
                            NoCharPlayerType::Unknown,
                        ),
                    );
                }
                // in worst case we have a snapshot that came before the player info arrived, still insert it with default infos
                else if !write_game_state.no_char_players.contains_key(&p.game_el_id) {
                    write_game_state.no_char_players.insert(
                        p.game_el_id.clone(),
                        NoCharPlayer::new(
                            PlayerInfo::default(),
                            Default::default(),
                            &p.game_el_id,
                            NoCharPlayerType::Unknown,
                        ),
                    );
                }

                // sort
                write_game_state.no_char_players.to_back(&p.game_el_id);
            });

            Self::convert_to_game_stages(
                &snapshot.stages,
                &mut write_game_state.stages,
                &mut write_game_state.world_pool,
                &write_game_state.game_objects_definitions,
                &write_game_state.id_generator,
                &write_game_state.game_options,
                &write_game_state.log,
                &write_game_state.players,
                &write_game_state.no_char_players,
                &mut write_game_state.unknown_players,
                NonZeroU16::new(write_game_state.collision.get_playfield_width() as u16).unwrap(),
                NonZeroU16::new(write_game_state.collision.get_playfield_height() as u16).unwrap(),
            );

            // same for no char players
            let no_char_players = &snapshot.no_char_players;
            write_game_state
                .no_char_players
                .retain_with_order(hi_closure!(
                    [no_char_players: &PoolLinkedHashMap<GameEntityId, SnapshotNoCharPlayer>],
                    |id: &GameEntityId, _: &mut NoCharPlayer| -> bool {
                        if no_char_players.contains_key(&id) {
                            true
                        } else {
                            false
                        }
                    }
                ));

            snapshot.local_players
        }
    }
}
