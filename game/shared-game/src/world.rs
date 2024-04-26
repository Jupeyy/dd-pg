pub mod world {
    use std::{collections::HashSet, num::NonZeroU16, rc::Rc, sync::Arc};

    use base::linked_hash_map_view::{
        LinkedHashMapEntryAndRes, LinkedHashMapExceptView, LinkedHashMapIterExt,
    };
    use base_log::log::SystemLog;
    use hashlink::LinkedHashSet;
    use hiarc::Hiarc;
    use math::math::{
        closest_point_on_line, distance, distance_squared,
        vector::{ivec2, vec2},
    };
    use pool::{
        datatypes::{PoolLinkedHashSet, PoolVec},
        pool::Pool,
    };

    use game_interface::types::{
        flag::FlagType,
        game::{GameEntityId, GameTickType},
        id_gen::IdGenerator,
        input::{CharacterInput, CharacterInputConsumableDiff},
        pickup::PickupType,
        weapons::WeaponType,
    };
    use num_traits::FromPrimitive;
    use serde::{Deserialize, Serialize};

    use crate::{
        collision::collision::Collision,
        entities::{
            character::{
                character::{self, CharacterPlayerTy, CharacterPool, CharactersView},
                core::character_core::{Core, CoreReusable},
                hook::character_hook::HookedCharacters,
                player::player::PlayerInfo,
                pos::character_pos::{CharacterPos, CharacterPositionPlayfield},
            },
            entity::entity::{EntityInterface, EntityTickResult},
            flag::flag::{Flag, FlagPool, Flags},
            laser::laser::{Laser, LaserPool, Lasers, WorldLaser},
            pickup::pickup::{Pickup, PickupPool, Pickups},
            projectile::projectile::{Projectile, ProjectilePool, WorldProjectile},
        },
        events::events::{CharacterEvent, FlagEvent, PickupEvent},
        game_objects::game_objects::{GameObjectDefinitions, GameObjectDefinitionsBase},
        simulation_pipe::simulation_pipe::{
            SimulationEntityEvents, SimulationEventWorldEntity, SimulationPipeFlag,
            SimulationPipeLaser, SimulationPipePickup, SimulationPipeProjectile,
        },
        spawns::GameSpawns,
        state::state::TICKS_PER_SECOND,
        types::types::{GameOptions, GameTeam},
    };

    use super::super::{
        entities::{
            character::character::{Character, Characters},
            projectile::projectile::Projectiles,
        },
        simulation_pipe::simulation_pipe::{
            SimulationPipeCharacter, SimulationPipeCharactersGetter, SimulationPipeStage,
        },
    };

    struct GetCharacterHelper<'a> {
        pub other_characters: LinkedHashMapExceptView<'a, GameEntityId, Character>,
        pub removed_characters: &'a mut LinkedHashSet<GameEntityId>,
    }

    impl<'a> SimulationPipeCharactersGetter for GetCharacterHelper<'a> {
        fn for_other_characters_in_range(
            &mut self,
            char_pos: &vec2,
            radius: f32,
            for_each_func: &mut dyn FnMut(&mut Character, &mut LinkedHashSet<GameEntityId>),
        ) {
            self.other_characters
                .iter_mut()
                .filter(|(_, char)| {
                    let other_pos = *char.pos.pos();

                    if distance(&other_pos, char_pos) < radius + character::PHYSICAL_SIZE {
                        true
                    } else {
                        false
                    }
                })
                .for_each(|(_, char)| for_each_func(char, self.removed_characters));
        }

        fn get_other_character_id_and_cores_iter_by_ids_mut(
            &mut self,
            ids: &HashSet<GameEntityId>,
            for_each_func: &mut dyn FnMut(
                &GameEntityId,
                &mut Core,
                &mut CoreReusable,
                &mut CharacterPos,
            ),
        ) {
            ids.iter().for_each(|id| {
                if !self.removed_characters.contains(id) {
                    if let Some(char) = self.other_characters.get_mut(id) {
                        let (core, reusable_core) = (&mut char.core, &mut char.reusable_core);
                        for_each_func(id, &mut core.core, &mut reusable_core.core, &mut char.pos)
                    }
                }
            });
        }

        fn get_other_character_pos_by_id(&self, other_char_id: &GameEntityId) -> &vec2 {
            self.other_characters.get(other_char_id).unwrap().pos.pos()
        }

        fn get_other_character_by_id_mut(
            &mut self,
            other_char_id: &GameEntityId,
        ) -> &mut Character {
            self.other_characters.get_mut(other_char_id).unwrap()
        }

        fn kill_character(&mut self, char_id: &GameEntityId) {
            self.removed_characters.insert(*char_id);
        }
    }

    #[derive(Debug, Hiarc, Clone)]
    pub struct WorldPool {
        removed_characters_helper_pool: Pool<LinkedHashSet<GameEntityId>>,
        projectile_pool: ProjectilePool,
        flag_pool: FlagPool,
        pickup_pool: PickupPool,
        laser_pool: LaserPool,
        character_pool: CharacterPool,
    }

    impl WorldPool {
        pub fn new(max_characters: usize) -> Self {
            Self {
                removed_characters_helper_pool: Pool::with_capacity(max_characters),
                projectile_pool: ProjectilePool {
                    projectile_pool: Pool::with_capacity(1024), // TODO: add hint for this
                    projectile_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                    projectile_helper: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                },
                flag_pool: FlagPool {
                    flag_pool: Pool::with_capacity(16), // TODO: add hint for this
                    flag_reusable_cores_pool: Pool::with_capacity(16 * 2), // TODO: add hint for this
                },
                pickup_pool: PickupPool {
                    pickup_pool: Pool::with_capacity(1024), // TODO: add hint for this
                    pickup_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                },
                laser_pool: LaserPool {
                    laser_pool: Pool::with_capacity(1024), // TODO: add hint for this
                    laser_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                },
                character_pool: CharacterPool {
                    character_pool: Pool::with_capacity(max_characters),
                    // reusable cores are used in snapshots quite frequently, and thus worth being pooled
                    // multiply by 2, because every character has two cores of this type
                    character_reusable_cores_pool: Pool::with_capacity(max_characters * 2),
                },
            }
        }
    }

    #[derive(Debug, Hiarc, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct GameObjectWorld {
        pub pos: ivec2,
        pub respawn_in_ticks: GameTickType,
    }
    pub type GameObjectsWorld = GameObjectDefinitionsBase<GameObjectWorld>;

    #[derive(Debug, Hiarc)]
    pub struct GameWorld {
        pub(crate) projectiles: Projectiles,
        pub(crate) flags: Flags,
        pub(crate) pickups: Pickups,
        pub(crate) lasers: Lasers,
        pub(crate) characters: Characters,

        /// inactive / non spawned / whatever game objects
        pub(crate) inactive_game_objects: GameObjectsWorld,

        removed_characters_helper: PoolLinkedHashSet<GameEntityId>,

        pub(crate) world_pool: WorldPool,

        pub(crate) id_gen: IdGenerator,

        simulation_events: SimulationEntityEvents,
        pub(crate) play_field: CharacterPositionPlayfield,
        pub(crate) hooks: HookedCharacters,

        pub(crate) log: Arc<SystemLog>,
    }

    impl GameWorld {
        pub fn new(
            world_pool: &WorldPool,
            game_object_definitions: &Rc<GameObjectDefinitions>,
            width: NonZeroU16,
            height: NonZeroU16,
            id_gen: &IdGenerator,
            log: &Arc<SystemLog>,
        ) -> Self {
            let simulation_events = SimulationEntityEvents::new();
            let mut inactive_game_objects = GameObjectsWorld {
                pickups: Default::default(),
            };

            let mut pickups = world_pool.pickup_pool.pickup_pool.new();
            let mut add_pick = |pickup_pos: &ivec2, ty: PickupType| {
                let id = id_gen.get_next();
                pickups.insert(
                    id,
                    Pickup::new(
                        &id,
                        log.logger("pickup"),
                        &(vec2::new(pickup_pos.x as f32, pickup_pos.y as f32) * 32.0
                            + vec2::new(16.0, 16.0)),
                        ty,
                        &world_pool.pickup_pool,
                        &simulation_events,
                    ),
                );
            };
            for pickup in &game_object_definitions.pickups.hearts {
                add_pick(pickup, PickupType::PowerupHealth);
            }
            for pickup in &game_object_definitions.pickups.shields {
                add_pick(pickup, PickupType::PowerupArmor);
            }
            for (index, weapons) in game_object_definitions.pickups.weapons.iter().enumerate() {
                for pickup in weapons {
                    add_pick(
                        pickup,
                        PickupType::PowerupWeapon(WeaponType::from_u32(index as u32).unwrap()),
                    );
                }
            }
            for pickup in &game_object_definitions.pickups.ninjas {
                inactive_game_objects.pickups.ninjas.push(GameObjectWorld {
                    pos: *pickup,
                    respawn_in_ticks: TICKS_PER_SECOND * 1, // TODO: * 90
                });
            }

            let mut flags = world_pool.flag_pool.flag_pool.new();
            let mut add_flag = |pos: &ivec2, ty: FlagType| {
                let id = id_gen.get_next();
                flags.insert(
                    id,
                    Flag::new(
                        &id,
                        log.logger("flag"),
                        &(vec2::new(pos.x as f32, pos.y as f32) * 32.0 + vec2::new(16.0, 16.0)),
                        ty,
                        &world_pool.flag_pool,
                        &simulation_events,
                    ),
                );
            };
            for flag in &game_object_definitions.pickups.red_flags {
                add_flag(flag, FlagType::Red)
            }
            for flag in &game_object_definitions.pickups.blue_flags {
                add_flag(flag, FlagType::Blue)
            }

            Self {
                removed_characters_helper: world_pool.removed_characters_helper_pool.new(),

                projectiles: world_pool.projectile_pool.projectile_pool.new(),
                flags,
                pickups,
                lasers: world_pool.laser_pool.laser_pool.new(),
                characters: world_pool.character_pool.character_pool.new(),

                inactive_game_objects,

                world_pool: world_pool.clone(),

                id_gen: id_gen.clone(),

                simulation_events,
                play_field: CharacterPositionPlayfield::new(width, height),
                hooks: Default::default(),

                log: log.clone(),
            }
        }

        pub fn add_character(
            &mut self,
            character_id: GameEntityId,
            stage_id: &GameEntityId,
            player_info: PlayerInfo,
            player_input: CharacterInput,
            game_options: &GameOptions,
            ty: CharacterPlayerTy,
            pos: vec2,
        ) -> &mut Character {
            self.characters.insert(
                character_id,
                Character::new(
                    &character_id,
                    &self.log,
                    &self.world_pool.character_pool,
                    player_info,
                    player_input,
                    &self.simulation_events,
                    game_options,
                    &stage_id,
                    ty,
                    pos,
                    &self.play_field,
                    &self.hooks,
                ),
            );
            self.characters.values_mut().last().unwrap()
        }

        /// returns closest distance, intersection position and the character
        pub fn intersect_character_on_line<'a, F>(
            field: &CharacterPositionPlayfield,
            mut characters: CharactersView<'a, F>,
            pos0: &vec2,
            pos1: &vec2,
            radius: f32,
        ) -> Option<(f32, vec2, &'a mut Character)>
        where
            F: Fn(&GameEntityId) -> bool,
        {
            let line_len = distance(pos0, pos1);
            let mut closest_distance = line_len * 100.0;
            let mut closest_intersect_pos: vec2 = Default::default();
            let mut intersect_char: Option<&GameEntityId> = None;

            let ids = field.by_radius(pos0, line_len);

            ids.iter().for_each(|id| {
                if let Some(char) = characters.get_mut(id) {
                    let char_pos = *char.pos.pos();
                    let mut intersect_pos = vec2::default();
                    if closest_point_on_line(&pos0, &pos1, &char_pos, &mut intersect_pos) {
                        let d = distance(&char_pos, &intersect_pos);
                        if d < character::PHYSICAL_SIZE + radius {
                            let d = distance(&pos0, &intersect_pos);
                            if d < closest_distance {
                                closest_intersect_pos = intersect_pos;
                                closest_distance = d;
                                intersect_char = Some(id);
                            }
                        }
                    }
                }
            });

            intersect_char.map(move |id| {
                (
                    closest_distance,
                    closest_intersect_pos,
                    characters.into_inner().0.get_mut(&id).unwrap(),
                )
            })
        }

        /// returns the intersected character
        pub fn intersect_character<'a, F>(
            field: &CharacterPositionPlayfield,
            mut characters: CharactersView<'a, F>,
            pos: &vec2,
            radius: f32,
        ) -> Option<&'a mut Character>
        where
            F: Fn(&GameEntityId) -> bool,
        {
            let mut closest_distance = f32::MAX;
            let mut intersect_char: Option<&GameEntityId> = None;

            let ids = field.by_radius(pos, radius);

            ids.iter().for_each(|id| {
                if let Some(char) = characters.get_mut(id) {
                    let char_pos = *char.pos.pos();
                    let d = distance(&char_pos, &pos);
                    if d < character::PHYSICAL_SIZE + radius {
                        if d < closest_distance {
                            closest_distance = d;
                            intersect_char = Some(id);
                        }
                    }
                }
            });

            intersect_char.map(|id| characters.into_inner().0.get_mut(id).unwrap())
        }

        /// returns the intersected characters
        pub fn intersect_characters<'a, 'b, F>(
            field: &'b CharacterPositionPlayfield,
            characters: CharactersView<'a, F>,
            pos: &'b vec2,
            radius: f32,
        ) -> impl Iterator<Item = &'a mut Character> + 'b
        where
            F: Fn(&GameEntityId) -> bool + 'b,
            'a: 'b,
        {
            let ids = field.by_radius(pos, radius);

            let (map, filter) = characters.into_inner();
            let view = CharactersView::new(map, move |id| filter(id) && ids.contains(id));

            view.into_iter().filter_map(move |(_, char)| {
                let char_pos = *char.pos.pos();
                let d = distance(&char_pos, &pos);
                (d < character::PHYSICAL_SIZE + radius).then_some(char)
            })
        }

        pub fn get_projectiles(&self) -> &Projectiles {
            &self.projectiles
        }

        pub fn get_lasers(&self) -> &Lasers {
            &self.lasers
        }

        pub fn get_pickups(&self) -> &Pickups {
            &self.pickups
        }

        pub fn get_flags(&self) -> &Flags {
            &self.flags
        }

        pub fn insert_new_projectile(
            &mut self,
            projectile_id: GameEntityId,
            owner_character_id: GameEntityId,

            pos: &vec2,
            direction: &vec2,
            life_span: i32,
            damage: u32,
            force: f32,
            explosive: bool,
            ty: WeaponType,
        ) {
            let projectile = Projectile::new(
                &projectile_id,
                self.log.logger("projectile"),
                pos,
                direction,
                life_span,
                damage,
                force,
                explosive,
                ty,
                &self.world_pool.projectile_pool,
                &self.simulation_events,
            );
            self.projectiles.insert(
                projectile_id.clone(),
                WorldProjectile {
                    character_id: owner_character_id,
                    projectile: projectile,
                },
            );
        }

        pub fn insert_new_laser(
            &mut self,
            laser_id: GameEntityId,
            owner_character_id: GameEntityId,

            pos: &vec2,
            dir: &vec2,
            start_energy: f32,

            can_hit_others: bool,
            can_hit_own: bool,
        ) {
            let laser = Laser::new(
                &laser_id,
                self.log.logger("laser"),
                pos,
                dir,
                start_energy,
                can_hit_others,
                can_hit_own,
                &self.world_pool.laser_pool,
                &self.simulation_events,
            );
            self.lasers.insert(
                laser_id.clone(),
                WorldLaser {
                    character_id: owner_character_id,
                    laser: laser,
                },
            );
        }

        pub fn insert_new_pickup(&mut self, pickup_id: GameEntityId, pos: &vec2, ty: PickupType) {
            self.pickups.insert(
                pickup_id.clone(),
                Pickup::new(
                    &pickup_id,
                    self.log.logger("pickup"),
                    pos,
                    ty,
                    &self.world_pool.pickup_pool,
                    &self.simulation_events,
                ),
            );
        }

        pub fn insert_new_flag(&mut self, flag_id: GameEntityId, pos: &vec2, ty: FlagType) {
            self.flags.insert(
                flag_id.clone(),
                Flag::new(
                    &flag_id,
                    self.log.logger("flag"),
                    pos,
                    ty,
                    &self.world_pool.flag_pool,
                    &self.simulation_events,
                ),
            );
        }

        fn tick_projectiles(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.retain_with_order(|_, proj| {
                if self.characters.contains_key(&proj.character_id) {
                    proj.projectile.tick(&mut SimulationPipeProjectile::new(
                        pipe.collision,
                        &mut self.characters,
                        proj.character_id.clone(),
                        &self.play_field,
                    )) != EntityTickResult::RemoveEntity
                } else {
                    false
                }
            });
        }

        fn post_tick_projectiles(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.retain_with_order(|_, proj| {
                proj.projectile
                    .tick_deferred(&mut SimulationPipeProjectile::new(
                        pipe.collision,
                        &mut self.characters,
                        proj.character_id.clone(),
                        &self.play_field,
                    ))
                    != EntityTickResult::RemoveEntity
            });
        }

        fn tick_flags(&mut self, pipe: &mut SimulationPipeStage) {
            self.flags.retain_with_order(|_, flag| {
                flag.tick(&mut SimulationPipeFlag::new(
                    pipe.collision,
                    &mut self.characters,
                    &self.play_field,
                )) != EntityTickResult::RemoveEntity
            });
        }

        fn post_tick_flags(&mut self, pipe: &mut SimulationPipeStage) {
            self.flags.retain_with_order(|_, flag| {
                flag.tick_deferred(&mut SimulationPipeFlag::new(
                    pipe.collision,
                    &mut self.characters,
                    &self.play_field,
                )) != EntityTickResult::RemoveEntity
            })
        }

        fn tick_pickups(&mut self, pipe: &mut SimulationPipeStage) {
            self.pickups.retain_with_order(|_, pickup| {
                pickup.tick(&mut SimulationPipePickup::new(
                    &mut self.characters,
                    &self.play_field,
                )) != EntityTickResult::RemoveEntity
            });
        }

        fn post_tick_pickups(&mut self, pipe: &mut SimulationPipeStage) {
            self.pickups.retain_with_order(|_, pickup| {
                pickup.tick_deferred(&mut SimulationPipePickup::new(
                    &mut self.characters,
                    &self.play_field,
                )) != EntityTickResult::RemoveEntity
            });
        }

        fn tick_lasers(&mut self, pipe: &mut SimulationPipeStage) {
            self.lasers.retain_with_order(|_, laser| {
                self.characters.contains_key(&laser.character_id)
                    && laser.laser.tick(&mut SimulationPipeLaser::new(
                        pipe.collision,
                        &mut self.characters,
                        laser.character_id,
                        &self.play_field,
                    )) != EntityTickResult::RemoveEntity
            });
        }

        fn post_tick_lasers(&mut self, pipe: &mut SimulationPipeStage) {
            self.lasers.retain_with_order(|_, laser| {
                laser.laser.tick_deferred(&mut SimulationPipeLaser::new(
                    pipe.collision,
                    &mut self.characters,
                    laser.character_id,
                    &self.play_field,
                )) != EntityTickResult::RemoveEntity
            });
        }

        fn tick_characters(&mut self, pipe: &mut SimulationPipeStage) {
            let mut characters = LinkedHashMapIterExt::new(&mut self.characters).rev();
            characters.for_each(|(id, (character, other_chars))| {
                let res = character.pre_tick(&mut SimulationPipeCharacter::new(
                    &mut GetCharacterHelper {
                        other_characters: other_chars,
                        removed_characters: &mut self.removed_characters_helper,
                    },
                    pipe.collision,
                ));
                if EntityTickResult::RemoveEntity == res {
                    self.removed_characters_helper.insert(id.clone());
                }
            });
            let mut characters = LinkedHashMapIterExt::new(&mut self.characters).rev();
            characters.for_each(|(id, (character, other_chars))| {
                let res = character.tick(&mut SimulationPipeCharacter::new(
                    &mut GetCharacterHelper {
                        other_characters: other_chars,
                        removed_characters: &mut self.removed_characters_helper,
                    },
                    pipe.collision,
                ));
                if EntityTickResult::RemoveEntity == res {
                    self.removed_characters_helper.insert(id.clone());
                }

                // handle the entity events
                character.entity_events.drain(..).for_each(|ev| {
                    match &ev {
                        CharacterEvent::Despawn { .. } => {
                            self.removed_characters_helper.insert(id.clone());
                        }
                        CharacterEvent::Projectile {
                            pos,
                            dir,
                            ty,
                            lifetime,
                        } => {
                            let proj_id = pipe.id_generator.get_next();
                            let projectile = Projectile::new(
                                &proj_id,
                                self.log.logger("projectile"),
                                pos,
                                dir,
                                (lifetime * TICKS_PER_SECOND as f32) as i32,
                                1,
                                0.0,
                                match ty {
                                    WeaponType::Hammer
                                    | WeaponType::Gun
                                    | WeaponType::Shotgun
                                    | WeaponType::Laser => false,
                                    WeaponType::Grenade => true,
                                },
                                *ty,
                                &mut pipe.world_pool.projectile_pool,
                                &self.simulation_events,
                            );
                            self.projectiles.insert(
                                proj_id.clone(),
                                WorldProjectile {
                                    character_id: character.base.game_element_id.clone(),
                                    projectile,
                                },
                            );
                        }
                        CharacterEvent::Laser { pos, dir, energy } => {
                            let id = pipe.id_generator.get_next();
                            let laser = Laser::new(
                                &id,
                                self.log.logger("laser"),
                                &pos,
                                &dir,
                                *energy,
                                true,  // TODO:
                                false, // TODO:
                                &mut pipe.world_pool.laser_pool,
                                &self.simulation_events,
                            );
                            self.lasers.insert(
                                id.clone(),
                                WorldLaser {
                                    character_id: character.base.game_element_id.clone(),
                                    laser,
                                },
                            );
                        }
                        _ => {}
                    }

                    self.simulation_events
                        .push(SimulationEventWorldEntity::Character {
                            player_id: id.clone(),
                            ev,
                        });
                });
            });
        }

        fn post_tick_characters(&mut self, pipe: &mut SimulationPipeStage) {
            let mut characters = LinkedHashMapIterExt::new(&mut self.characters).rev();
            characters.for_each(|(id, (character, other_chars))| {
                let res = character.tick_deferred(&mut SimulationPipeCharacter::new(
                    &mut GetCharacterHelper {
                        other_characters: other_chars,
                        removed_characters: &mut self.removed_characters_helper,
                    },
                    pipe.collision,
                ));
                if EntityTickResult::RemoveEntity == res {
                    self.removed_characters_helper.insert(id.clone());
                }
            });
        }

        pub fn handle_character_input_change(
            &mut self,
            collision: &Collision,
            id: &GameEntityId,
            diff: CharacterInputConsumableDiff,
        ) {
            let (character, other_chars) = LinkedHashMapEntryAndRes::get(&mut self.characters, id);
            let res = character.handle_input_change(
                &mut SimulationPipeCharacter::new(
                    &mut GetCharacterHelper {
                        other_characters: other_chars,
                        removed_characters: &mut self.removed_characters_helper,
                    },
                    collision,
                ),
                diff,
            );
            if EntityTickResult::RemoveEntity == res {
                self.removed_characters_helper.insert(id.clone());
            }
        }

        fn handle_removed_entities(&mut self) {
            self.removed_characters_helper.drain().for_each(|id| {
                self.characters.remove(&id);
            });
        }

        pub(crate) fn get_spawn_pos(&self, spawns: &GameSpawns, team: Option<GameTeam>) -> vec2 {
            match team {
                Some(team) => match team {
                    GameTeam::Red => todo!(),
                    GameTeam::Blue => todo!(),
                },
                None => {
                    // find spawn furthest away from all players
                    // reverse iterators bcs if multiple are found the first should be
                    // picked, not the last
                    spawns
                        .spawns
                        .iter()
                        .rev()
                        .chain(spawns.spawns_red.iter().rev())
                        .chain(spawns.spawns_blue.iter().rev())
                        .max_by(|&spawn1, &spawn2| {
                            let sum_dist_spawn = |spawn: &vec2| {
                                self.characters
                                    .values()
                                    .map(|char| distance_squared(spawn, char.pos.pos()) as f64)
                                    .sum::<f64>()
                            };

                            sum_dist_spawn(spawn1).total_cmp(&sum_dist_spawn(spawn2))
                        })
                        .cloned()
                        .unwrap_or(vec2::default())
                }
            }
        }

        fn handle_simulation_events(&mut self, events: &Vec<SimulationEventWorldEntity>) {
            for ev in events.iter() {
                match ev {
                    SimulationEventWorldEntity::Character { .. }
                    | SimulationEventWorldEntity::Projectile { .. }
                    | SimulationEventWorldEntity::Laser { .. } => {
                        // ignore
                    }
                    SimulationEventWorldEntity::Pickup { ev, .. } => match ev {
                        PickupEvent::Despawn { pos, ty, .. } => {
                            let pos = ivec2::new((pos.x / 32.0) as i32, (pos.y / 32.0) as i32);
                            let respawn_ticks = TICKS_PER_SECOND * 15;
                            match ty {
                                PickupType::PowerupHealth => {
                                    self.inactive_game_objects.pickups.hearts.push(
                                        GameObjectWorld {
                                            pos,
                                            respawn_in_ticks: respawn_ticks,
                                        },
                                    )
                                }
                                PickupType::PowerupArmor => {
                                    self.inactive_game_objects.pickups.shields.push(
                                        GameObjectWorld {
                                            pos,
                                            respawn_in_ticks: respawn_ticks,
                                        },
                                    )
                                }
                                PickupType::PowerupNinja => {
                                    self.inactive_game_objects.pickups.ninjas.push(
                                        GameObjectWorld {
                                            pos,
                                            respawn_in_ticks: TICKS_PER_SECOND * 90,
                                        },
                                    )
                                }
                                PickupType::PowerupWeapon(weapon) => {
                                    self.inactive_game_objects.pickups.weapons[*weapon as usize]
                                        .push(GameObjectWorld {
                                            pos,
                                            respawn_in_ticks: respawn_ticks,
                                        })
                                }
                            }
                        }
                        PickupEvent::Pickup { .. } => {
                            // ignore
                        }
                    },
                    SimulationEventWorldEntity::Flag { ev, .. } => match ev {
                        FlagEvent::Despawn { pos, ty, .. } => {
                            let pos = ivec2::new((pos.x / 32.0) as i32, (pos.y / 32.0) as i32);
                            let respawn_ticks = TICKS_PER_SECOND * 15;
                            match ty {
                                FlagType::Red => self.inactive_game_objects.pickups.red_flags.push(
                                    GameObjectWorld {
                                        pos,
                                        respawn_in_ticks: respawn_ticks,
                                    },
                                ),
                                FlagType::Blue => {
                                    self.inactive_game_objects.pickups.blue_flags.push(
                                        GameObjectWorld {
                                            pos,
                                            respawn_in_ticks: respawn_ticks,
                                        },
                                    )
                                }
                            }
                        }
                        FlagEvent::Sound { .. } | FlagEvent::Effect { .. } => {
                            // ignore
                        }
                    },
                }
            }
        }

        fn check_inactive_game_objects(&mut self) {
            let mut add_pickup = |obj: &mut GameObjectWorld, ty: PickupType| {
                obj.respawn_in_ticks -= 1;
                if obj.respawn_in_ticks == 0 {
                    let pos = vec2::new(obj.pos.x as f32, obj.pos.y as f32) * 32.0
                        + vec2::new(16.0, 16.0);
                    let id = self.id_gen.get_next();
                    self.pickups.insert(
                        id.clone(),
                        Pickup::new(
                            &id,
                            self.log.logger("pickup"),
                            &pos,
                            ty,
                            &self.world_pool.pickup_pool,
                            &self.simulation_events,
                        ),
                    );
                    false
                } else {
                    true
                }
            };
            self.inactive_game_objects
                .pickups
                .hearts
                .retain_mut(|obj| add_pickup(obj, PickupType::PowerupHealth));
            self.inactive_game_objects
                .pickups
                .shields
                .retain_mut(|obj| add_pickup(obj, PickupType::PowerupArmor));
            self.inactive_game_objects
                .pickups
                .ninjas
                .retain_mut(|obj| add_pickup(obj, PickupType::PowerupNinja));
            self.inactive_game_objects
                .pickups
                .weapons
                .iter_mut()
                .enumerate()
                .for_each(|(ty, weapons)| {
                    let ty = WeaponType::from_usize(ty).unwrap();
                    weapons.retain_mut(|obj| add_pickup(obj, PickupType::PowerupWeapon(ty)));
                });
        }

        #[must_use]
        pub fn tick(
            &mut self,
            pipe: &mut SimulationPipeStage,
        ) -> PoolVec<SimulationEventWorldEntity> {
            self.check_inactive_game_objects();

            self.tick_characters(pipe);
            self.tick_projectiles(pipe);
            self.tick_flags(pipe);
            self.tick_pickups(pipe);
            self.tick_lasers(pipe);

            self.post_tick_characters(pipe);
            self.post_tick_projectiles(pipe);
            self.post_tick_flags(pipe);
            self.post_tick_pickups(pipe);
            self.post_tick_lasers(pipe);

            if !pipe.is_prediction {
                self.handle_removed_entities();
            } else {
                self.removed_characters_helper.clear();
            }
            let events = self.simulation_events.take();

            self.handle_simulation_events(&events);

            events
        }
    }
}
