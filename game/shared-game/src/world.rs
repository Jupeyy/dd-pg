pub mod world {
    use std::sync::Arc;

    use base::linked_hash_map_view::{LinkedHashMapIterExt, LinkedHashMapView};
    use base_log::log::SystemLog;
    use hashlink::LinkedHashSet;
    use math::math::{closest_point_on_line, distance, vector::vec2};
    use pool::{datatypes::PoolLinkedHashSet, pool::Pool};

    use shared_base::{
        game_types::TGameElementID,
        network::messages::{MsgObjPlayerInfo, WeaponType},
        types::GameTickType,
    };

    use crate::{
        entities::{
            character::character::{self, CharacterPool},
            entity::entity::EntityInterface,
            flag::flag::{Flag, FlagPool, Flags},
            laser::laser::{Laser, LaserPool, Lasers, WorldLaser},
            pickup::pickup::{Pickup, PickupPool, Pickups},
            projectile::projectile::{Projectile, ProjectilePool, WorldProjectile},
        },
        events::events::{CharacterEvent, FlagEvent, LaserEvent, PickupEvent, ProjectileEvent},
        game_objects::game_objects::{
            WorldGameObjects, WorldGameObjectsPickups, WorldGameObjectsPickupsPool,
            WorldGameObjectsPool,
        },
        player::player::PlayerInfo,
        simulation_pipe::simulation_pipe::{
            SimulationEventsWorld, SimulationPipeFlag, SimulationPipeLaser, SimulationPipePickup,
            SimulationPipeProjectile,
        },
        state::state::TICKS_PER_SECOND,
        types::types::GameOptions,
    };

    use super::super::{
        entities::{
            character::character::{Character, Characters},
            character_core::character_core::{Core, CoreReusable},
            projectile::projectile::Projectiles,
        },
        simulation_pipe::simulation_pipe::{
            SimulationEvent, SimulationPipeCharacter, SimulationPipeCharactersGetter,
            SimulationPipeStage,
        },
    };

    struct GetCharacterHelper<'a> {
        pub other_characters: LinkedHashMapView<'a, TGameElementID, Character>,
        pub removed_characters: &'a LinkedHashSet<TGameElementID>,
    }

    impl<'a> SimulationPipeCharactersGetter for GetCharacterHelper<'a> {
        fn for_other_characters_in_range(
            &mut self,
            char_pos: &vec2,
            radius: f32,
            for_each_func: &mut dyn FnMut(&mut Character),
        ) {
            self.other_characters
                .iter_mut()
                .filter(|(_, char)| {
                    let other_pos = char.core.core.pos;

                    if distance(&other_pos, char_pos) < radius + character::PHYSICAL_SIZE {
                        true
                    } else {
                        false
                    }
                })
                .for_each(|(_, char)| for_each_func(char));
        }

        fn get_other_character_id_and_cores_iter(
            &self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &Core),
        ) {
            self.other_characters
                .iter()
                .filter(|(id, _)| !self.removed_characters.contains(id))
                .map(|(id, char)| (id, &char.core.core))
                .for_each(|(id, core)| for_each_func(id, core))
        }

        fn get_other_character_id_and_cores_iter_mut(
            &mut self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &mut Core, &mut CoreReusable),
        ) {
            self.other_characters
                .iter_mut()
                .filter(|(id, _)| !self.removed_characters.contains(id))
                .map(|(id, char)| {
                    let (_, core, reusable_core) = char.split_mut();
                    (id, &mut core.core, &mut reusable_core.core)
                })
                .for_each(|(id, core, reusable_core)| for_each_func(id, core, reusable_core))
        }

        fn get_other_character_core_by_id(&self, other_char_id: &TGameElementID) -> &Core {
            &self.other_characters.get(other_char_id).unwrap().core.core
        }

        fn get_other_character_by_id_mut(
            &mut self,
            other_char_id: &TGameElementID,
        ) -> &mut Character {
            self.other_characters.get_mut(other_char_id).unwrap()
        }
    }

    #[derive(Debug, Clone)]
    pub struct WorldPool {
        removed_projectiles_helper_pool: Pool<LinkedHashSet<TGameElementID>>,
        removed_flags_helper_pool: Pool<LinkedHashSet<TGameElementID>>,
        removed_pickups_helper_pool: Pool<LinkedHashSet<TGameElementID>>,
        removed_lasers_helper_pool: Pool<LinkedHashSet<TGameElementID>>,
        removed_characters_helper_pool: Pool<LinkedHashSet<TGameElementID>>,
        projectile_pool: ProjectilePool,
        flag_pool: FlagPool,
        pickup_pool: PickupPool,
        laser_pool: LaserPool,
        character_pool: CharacterPool,
        game_obj_pool: WorldGameObjectsPool,
    }

    impl WorldPool {
        pub fn new(max_characters: usize) -> Self {
            Self {
                removed_characters_helper_pool: Pool::with_capacity(max_characters),
                removed_projectiles_helper_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                removed_flags_helper_pool: Pool::with_capacity(16 * 2), // TODO: add hint for this
                removed_pickups_helper_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                removed_lasers_helper_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                projectile_pool: ProjectilePool {
                    projectile_pool: Pool::with_capacity(1024), // TODO: add hint for this
                    projectile_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
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
                game_obj_pool: WorldGameObjectsPool {
                    pickups: WorldGameObjectsPickupsPool {
                        hearts: Pool::with_capacity(64), // TODO: hint for this (?)
                    },
                },
            }
        }
    }

    pub struct GameWorld {
        pub(crate) projectiles: Projectiles,
        pub(crate) flags: Flags,
        pub(crate) pickups: Pickups,
        pub(crate) lasers: Lasers,
        pub(crate) characters: Characters,

        pub(crate) game_objects: WorldGameObjects,

        removed_projectiles_helper: PoolLinkedHashSet<TGameElementID>,
        removed_flags_helper: PoolLinkedHashSet<TGameElementID>,
        removed_pickups_helper: PoolLinkedHashSet<TGameElementID>,
        removed_lasers_helper: PoolLinkedHashSet<TGameElementID>,
        removed_characters_helper: PoolLinkedHashSet<TGameElementID>,

        pub(crate) world_pool: WorldPool,

        pub(crate) log: Arc<SystemLog>,
    }

    impl GameWorld {
        pub fn new(world_pool: &WorldPool, log: &Arc<SystemLog>) -> Self {
            Self {
                removed_projectiles_helper: world_pool.removed_projectiles_helper_pool.new(),
                removed_flags_helper: world_pool.removed_flags_helper_pool.new(),
                removed_pickups_helper: world_pool.removed_pickups_helper_pool.new(),
                removed_lasers_helper: world_pool.removed_lasers_helper_pool.new(),
                removed_characters_helper: world_pool.removed_characters_helper_pool.new(),

                projectiles: world_pool.projectile_pool.projectile_pool.new(),
                flags: world_pool.flag_pool.flag_pool.new(),
                pickups: world_pool.pickup_pool.pickup_pool.new(),
                lasers: world_pool.laser_pool.laser_pool.new(),
                characters: world_pool.character_pool.character_pool.new(),

                game_objects: WorldGameObjects {
                    pickups: WorldGameObjectsPickups {
                        hearts: world_pool.game_obj_pool.pickups.hearts.new(),
                    },
                },

                world_pool: world_pool.clone(),

                log: log.clone(),
            }
        }

        pub fn add_character(
            &mut self,
            character_id: TGameElementID,
            player_info: MsgObjPlayerInfo,
            game_options: &GameOptions,
        ) -> &mut Character {
            self.characters.insert(
                character_id,
                Character::new(
                    &character_id,
                    self.log.logger("character"),
                    &self.world_pool.character_pool,
                    PlayerInfo {
                        player_info,
                        version: 0,
                    },
                    game_options,
                ),
            );
            self.characters.values_mut().last().unwrap()
        }

        /// returns closest distance, intersection position and the character
        pub fn intersect_character<'a>(
            characters: impl Iterator<Item = &'a mut Character>,
            pos0: &vec2,
            pos1: &vec2,
            radius: f32,
        ) -> Option<(f32, vec2, &'a mut Character)> {
            let mut closest_distance = distance(pos0, pos1) * 100.0;
            let mut closest_intersect_pos: vec2 = Default::default();
            let mut intersect_char: Option<&'a mut Character> = None;

            characters.for_each(|char| {
                let char_pos = char.core.core.pos;
                let mut intersect_pos = vec2::default();
                if closest_point_on_line(&pos0, &pos1, &char_pos, &mut intersect_pos) {
                    let d = distance(&char_pos, &intersect_pos);
                    if d < character::PHYSICAL_SIZE + radius {
                        let d = distance(&pos0, &intersect_pos);
                        if d < closest_distance {
                            closest_intersect_pos = intersect_pos;
                            closest_distance = d;
                            intersect_char = Some(char);
                        }
                    }
                }
            });

            intersect_char.map(|char| (closest_distance, closest_intersect_pos, char))
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
            projectile_id: TGameElementID,
            owner_character_id: TGameElementID,

            pos: &vec2,
            direction: &vec2,
            life_span: i32,
            damage: u32,
            force: f32,
            start_tick: GameTickType,
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
                start_tick,
                explosive,
                ty,
                &self.world_pool.projectile_pool,
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
            laser_id: TGameElementID,
            owner_character_id: TGameElementID,

            pos: &vec2,
            dir: &vec2,
            start_tick: GameTickType,
            start_energy: f32,

            can_hit_others: bool,
            can_hit_own: bool,
        ) {
            let laser = Laser::new(
                &laser_id,
                self.log.logger("laser"),
                pos,
                dir,
                start_tick,
                start_energy,
                can_hit_others,
                can_hit_own,
                &self.world_pool.laser_pool,
            );
            self.lasers.insert(
                laser_id.clone(),
                WorldLaser {
                    character_id: owner_character_id,
                    laser: laser,
                },
            );
        }

        pub fn insert_new_pickup(&mut self, pickup_id: TGameElementID, pos: &vec2) {
            self.pickups.insert(
                pickup_id.clone(),
                Pickup::new(
                    &pickup_id,
                    self.log.logger("pickup"),
                    pos,
                    &self.world_pool.pickup_pool,
                ),
            );
        }

        pub fn insert_new_flag(&mut self, flag_id: TGameElementID, pos: &vec2) {
            self.flags.insert(
                flag_id.clone(),
                Flag::new(
                    &flag_id,
                    self.log.logger("flag"),
                    pos,
                    &self.world_pool.flag_pool,
                ),
            );
        }

        fn tick_projectiles(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.values_mut().for_each(|proj| {
                proj.projectile.tick(&mut SimulationPipeProjectile::new(
                    pipe.collision,
                    &mut self.characters,
                    pipe.cur_tick,
                    proj.character_id.clone(),
                ));
                // handle the entity events
                let ent = proj.projectile.split_mut().0;
                let id = ent.game_element_id.clone();
                proj.projectile
                    .entity_events
                    .drain(..)
                    .for_each(|ev| match ev {
                        ProjectileEvent::Despawn { .. } => {
                            self.removed_projectiles_helper.insert(id.clone());
                        }
                        _ => {}
                    });
            });
        }

        fn post_tick_projectiles(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.values_mut().for_each(|proj| {
                proj.projectile
                    .tick_deferred(&mut SimulationPipeProjectile::new(
                        pipe.collision,
                        &mut self.characters,
                        pipe.cur_tick,
                        proj.character_id.clone(),
                    ));
            });
        }

        fn tick_flags(&mut self, pipe: &mut SimulationPipeStage) {
            self.flags.values_mut().for_each(|flag| {
                flag.tick(&mut SimulationPipeFlag::new());
                // handle the entity events
                let id = flag.base.game_element_id.clone();
                flag.entity_events.drain(..).for_each(|ev| match ev {
                    FlagEvent::Despawn { .. } => {
                        self.removed_flags_helper.insert(id.clone());
                    }
                    _ => {}
                });
            });
        }

        fn post_tick_flags(&mut self, pipe: &mut SimulationPipeStage) {
            self.flags.values_mut().for_each(|flag| {
                flag.tick_deferred(&mut SimulationPipeFlag::new());
            });
        }

        fn tick_pickups(&mut self, pipe: &mut SimulationPipeStage) {
            self.pickups.values_mut().for_each(|pickup| {
                pickup.tick(&mut SimulationPipePickup::new());
                // handle the entity events
                let ent = pickup.split_mut().0;
                let id = ent.game_element_id.clone();
                pickup.entity_events.drain(..).for_each(|ev| match ev {
                    PickupEvent::Despawn { .. } => {
                        self.removed_pickups_helper.insert(id.clone());
                    }
                    _ => {}
                });
            });
        }

        fn post_tick_pickups(&mut self, pipe: &mut SimulationPipeStage) {
            self.pickups.values_mut().for_each(|pickup| {
                pickup.tick_deferred(&mut SimulationPipePickup::new());
            });
        }

        fn tick_lasers(&mut self, pipe: &mut SimulationPipeStage) {
            self.lasers.values_mut().for_each(|laser| {
                laser.laser.tick(&mut SimulationPipeLaser::new(
                    pipe.cur_tick,
                    pipe.collision,
                    &mut self.characters,
                    laser.character_id,
                ));
                // handle the entity events
                let ent = laser.laser.split_mut().0;
                let id = ent.game_element_id.clone();
                laser.laser.entity_events.drain(..).for_each(|ev| match ev {
                    LaserEvent::Despawn { .. } => {
                        self.removed_lasers_helper.insert(id.clone());
                    }
                    _ => {}
                });
            });
        }

        fn post_tick_lasers(&mut self, pipe: &mut SimulationPipeStage) {
            self.lasers.values_mut().for_each(|laser| {
                laser.laser.tick_deferred(&mut SimulationPipeLaser::new(
                    pipe.cur_tick,
                    pipe.collision,
                    &mut self.characters,
                    laser.character_id,
                ));
            });
        }

        fn tick_characters(&mut self, pipe: &mut SimulationPipeStage) {
            let mut characters = LinkedHashMapIterExt::new(&mut self.characters);
            characters.for_each(|(id, (character, other_chars))| {
                character.tick(&mut SimulationPipeCharacter::new(
                    &mut GetCharacterHelper {
                        other_characters: other_chars,
                        removed_characters: &self.removed_characters_helper,
                    },
                    pipe.collision,
                    pipe.cur_tick,
                ));

                // handle the entity events
                character.entity_events.drain(..).for_each(|ev| {
                    match &ev {
                        CharacterEvent::Despawn { .. } => {
                            self.removed_characters_helper.insert(id.clone());
                        }
                        CharacterEvent::Projectile { pos, dir, ty } => {
                            let proj_id = pipe.id_generator.get_next();
                            let projectile = Projectile::new(
                                &proj_id,
                                self.log.logger("projectile"),
                                pos,
                                dir,
                                2 * TICKS_PER_SECOND as i32,
                                0,
                                0.0,
                                pipe.cur_tick,
                                false,
                                *ty,
                                &mut pipe.world_pool.projectile_pool,
                            );
                            self.projectiles.insert(
                                proj_id.clone(),
                                WorldProjectile {
                                    character_id: character.base.game_element_id.clone(),
                                    projectile,
                                },
                            );
                        }
                        CharacterEvent::Laser { pos, dir } => {
                            let id = pipe.id_generator.get_next();
                            let laser = Laser::new(
                                &id,
                                self.log.logger("laser"),
                                &pos,
                                &dir,
                                pipe.cur_tick,
                                800.0, // TODO:
                                true,  // TODO:
                                true,  // TODO:
                                &mut pipe.world_pool.laser_pool,
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

                    pipe.simulation_events.push(SimulationEvent::World {
                        stage_id: pipe.stage_id.clone(),
                        ev: SimulationEventsWorld::Character {
                            player_id: id.clone(),
                            ev,
                        },
                    });
                });
            });
        }

        fn post_tick_characters(&mut self, pipe: &mut SimulationPipeStage) {
            let mut characters = LinkedHashMapIterExt::new(&mut self.characters);
            characters.for_each(|(_, (character, other_chars))| {
                character.tick_deferred(&mut SimulationPipeCharacter::new(
                    &mut GetCharacterHelper {
                        other_characters: other_chars,
                        removed_characters: &self.removed_characters_helper,
                    },
                    pipe.collision,
                    pipe.cur_tick,
                ));
            });
        }

        fn handle_removed_entities(&mut self) {
            self.removed_characters_helper.drain().for_each(|id| {
                let core = &mut self.characters.get_mut(&id).unwrap().reusable_core.core;
                // TODO: swap with pool (or put the ids back to pool)
                let mut attached_character_ids = Default::default();
                std::mem::swap(
                    &mut core.hooked_character.attached_characters_ids,
                    &mut attached_character_ids,
                );

                attached_character_ids.iter().for_each(|attached_char_id| {
                    if let Some(attached_char) = self.characters.get_mut(attached_char_id) {
                        attached_char.reusable_core.core.hooked_character.id = None;
                    }
                });
                self.characters.remove(&id);
            });
            self.removed_projectiles_helper.drain().for_each(|id| {
                self.projectiles.remove(&id);
            });
            self.removed_flags_helper.drain().for_each(|id| {
                self.flags.remove(&id);
            });
            self.removed_pickups_helper.drain().for_each(|id| {
                self.pickups.remove(&id);
            });
            self.removed_lasers_helper.drain().for_each(|id| {
                self.lasers.remove(&id);
            });
        }

        pub fn tick(&mut self, pipe: &mut SimulationPipeStage) {
            self.tick_projectiles(pipe);
            self.tick_flags(pipe);
            self.tick_pickups(pipe);
            self.tick_lasers(pipe);
            self.tick_characters(pipe);

            self.post_tick_projectiles(pipe);
            self.post_tick_flags(pipe);
            self.post_tick_pickups(pipe);
            self.post_tick_lasers(pipe);
            self.post_tick_characters(pipe);

            if !pipe.is_prediction {
                self.handle_removed_entities();
            } else {
                self.removed_projectiles_helper.clear();
                self.removed_flags_helper.clear();
                self.removed_pickups_helper.clear();
                self.removed_lasers_helper.clear();
                self.removed_characters_helper.clear();
            }
        }
    }
}
