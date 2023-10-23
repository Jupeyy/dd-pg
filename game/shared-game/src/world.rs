pub mod world {
    use std::sync::Arc;

    use base_log::log::SystemLog;
    use hashlink::{LinkedHashMap, LinkedHashSet};
    use math::math::{closest_point_on_line, distance, vector::vec2};
    use pool::{
        datatypes::{PoolLinkedHashMap, PoolLinkedHashSet},
        pool::Pool,
    };

    use shared_base::{
        game_types::TGameElementID, id_gen::IDGenerator, network::messages::WeaponType,
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
        events::events::EntityEvent,
        simulation_pipe::simulation_pipe::{
            SimulationPipeFlag, SimulationPipeLaser, SimulationPipePickup, SimulationPipeProjectile,
        },
        state::state::TICKS_PER_SECOND,
    };

    use super::super::{
        entities::{
            character::character::{Character, Characters},
            character_core::character_core::{Core, CoreReusable},
            projectile::projectile::Projectiles,
        },
        player::player::{NoCharPlayerType, PlayerRemoveInfo},
        simulation_pipe::simulation_pipe::{
            SimulationEvents, SimulationPipeCharacter, SimulationPipeCharactersGetter,
            SimulationPipeStage,
        },
    };

    struct GetCharacterHelper<'a> {
        pub characters: &'a mut Characters,
        pub cur_character_id: &'a TGameElementID,
        pub removed_characters: &'a LinkedHashSet<TGameElementID>,
        pub cur_core_index: usize,
    }

    impl<'a> SimulationPipeCharactersGetter for GetCharacterHelper<'a> {
        fn get_character(&mut self) -> &mut Character {
            self.characters.get_mut(self.cur_character_id).unwrap()
        }

        fn get_character_id(&self) -> &TGameElementID {
            &self.cur_character_id
        }

        fn for_other_characters_in_range(
            &mut self,
            core_index: usize,
            pos: &vec2,
            radius: f32,
            for_each_func: &mut dyn FnMut(&mut Character),
        ) {
            let this_char_pos = self
                .characters
                .get(self.cur_character_id)
                .unwrap()
                .get_core_at_index(core_index)
                .core
                .pos;

            self.characters
                .iter_mut()
                .filter(|(char_id, char)| {
                    **char_id != *self.cur_character_id && {
                        let other_pos = char.get_core_at_index(core_index).core.pos;

                        if distance(&other_pos, &this_char_pos) < radius + character::PHYSICAL_SIZE
                        {
                            true
                        } else {
                            false
                        }
                    }
                })
                .for_each(|(_, char)| for_each_func(char));
        }

        fn get_other_character_id_and_cores_iter(
            &self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &Core),
        ) {
            self.characters
                .iter()
                .filter(|(id, _)| {
                    *id != self.cur_character_id && !self.removed_characters.contains(id)
                })
                .map(|(id, char)| (id, &char.get_core_at_index(self.cur_core_index).core))
                .for_each(|(id, core)| for_each_func(id, core))
        }

        fn get_other_character_id_and_cores_iter_mut(
            &mut self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &mut Core, &mut CoreReusable),
        ) {
            self.characters
                .iter_mut()
                .filter(|(id, _)| {
                    *id != self.cur_character_id && !self.removed_characters.contains(id)
                })
                .map(|(id, char)| {
                    let (_, core, reusable_core) = char.split_mut(self.cur_core_index);
                    (id, &mut core.core, &mut reusable_core.core)
                })
                .for_each(|(id, core, reusable_core)| for_each_func(id, core, reusable_core))
        }

        fn get_other_character_core_by_id(&self, other_char_id: &TGameElementID) -> &Core {
            &self
                .characters
                .get(other_char_id)
                .unwrap()
                .get_core_at_index(self.cur_core_index)
                .core
        }

        fn get_other_character_by_id_mut(
            &mut self,
            other_char_id: &TGameElementID,
        ) -> &mut Character {
            self.characters.get_mut(other_char_id).unwrap()
        }
    }

    pub struct WorldPool {
        removed_players_helper_pool: Pool<LinkedHashMap<TGameElementID, PlayerRemoveInfo>>,
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
    }

    impl WorldPool {
        pub fn new(max_characters: usize) -> Self {
            Self {
                removed_players_helper_pool: Pool::with_capacity(max_characters),
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
            }
        }
    }

    pub struct GameWorld {
        pub(crate) projectiles: Projectiles,
        pub(crate) flags: Flags,
        pub(crate) pickups: Pickups,
        pub(crate) lasers: Lasers,
        pub(crate) characters: Characters,

        removed_players_helper: PoolLinkedHashMap<TGameElementID, PlayerRemoveInfo>,
        removed_projectiles_helper: PoolLinkedHashSet<TGameElementID>,
        removed_flags_helper: PoolLinkedHashSet<TGameElementID>,
        removed_pickups_helper: PoolLinkedHashSet<TGameElementID>,
        removed_lasers_helper: PoolLinkedHashSet<TGameElementID>,
        removed_characters_helper: PoolLinkedHashSet<TGameElementID>,

        log: Arc<SystemLog>,
    }

    impl GameWorld {
        pub fn new(world_pool: &mut WorldPool, log: &Arc<SystemLog>) -> Self {
            Self {
                removed_players_helper: world_pool.removed_players_helper_pool.new(),
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

                log: log.clone(),
            }
        }

        pub fn get_character_by_id(&self, id: &TGameElementID) -> &Character {
            self.characters.get(id).unwrap()
        }

        pub fn get_character_by_id_mut(&mut self, id: &TGameElementID) -> &mut Character {
            self.characters.get_mut(id).unwrap()
        }

        pub fn get_characters(&self) -> &Characters {
            &self.characters
        }

        pub fn get_characters_mut(&mut self) -> &mut LinkedHashMap<TGameElementID, Character> {
            &mut self.characters
        }

        pub fn add_character(
            &mut self,
            game_el_gen: &mut IDGenerator,
            characters_pool: &mut WorldPool,
        ) -> &mut Character {
            let id = game_el_gen.get_next();
            self.characters.insert(
                id.clone(),
                Character::new(
                    &id,
                    self.log.logger("character"),
                    &mut characters_pool.character_pool,
                ),
            );
            self.characters.values_mut().last().unwrap()
        }

        pub fn insert_new_character(
            &mut self,
            character_id: TGameElementID,
            characters_pool: &mut WorldPool,
        ) {
            self.characters.insert(
                character_id.clone(),
                Character::new(
                    &character_id,
                    self.log.logger("character"),
                    &mut characters_pool.character_pool,
                ),
            );
        }

        pub fn rem_character(&mut self, char_id: &TGameElementID) -> Option<Character> {
            self.characters.remove(char_id)
        }

        /// returns closest distance, intersection position and the character
        pub fn intersect_character<'a>(
            characters: impl Iterator<Item = &'a mut Character>,
            cur_core_index: usize,
            pos0: &vec2,
            pos1: &vec2,
            radius: f32,
        ) -> Option<(f32, vec2, &'a mut Character)> {
            let mut closest_distance = distance(pos0, pos1) * 100.0;
            let mut closest_intersect_pos: vec2 = Default::default();
            let mut intersect_char: Option<&'a mut Character> = None;

            characters.for_each(|char| {
                let char_pos = char.get_core_at_index(cur_core_index).core.pos;
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

            world_pool: &mut WorldPool,
        ) {
            self.projectiles.insert(
                projectile_id.clone(),
                WorldProjectile {
                    character_id: owner_character_id,
                    projectile: Projectile::new(
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
                        &mut world_pool.projectile_pool,
                    ),
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

            world_pool: &mut WorldPool,
        ) {
            self.lasers.insert(
                laser_id.clone(),
                WorldLaser {
                    character_id: owner_character_id,
                    laser: Laser::new(
                        &laser_id,
                        self.log.logger("laser"),
                        pos,
                        dir,
                        start_tick,
                        start_energy,
                        can_hit_others,
                        can_hit_own,
                        &mut world_pool.laser_pool,
                    ),
                },
            );
        }

        pub fn insert_new_pickup(
            &mut self,
            pickup_id: TGameElementID,

            pos: &vec2,

            world_pool: &mut WorldPool,
        ) {
            self.pickups.insert(
                pickup_id.clone(),
                Pickup::new(
                    &pickup_id,
                    self.log.logger("pickup"),
                    pos,
                    &mut world_pool.pickup_pool,
                ),
            );
        }

        pub fn insert_new_flag(
            &mut self,
            flag_id: TGameElementID,

            pos: &vec2,

            world_pool: &mut WorldPool,
        ) {
            self.flags.insert(
                flag_id.clone(),
                Flag::new(
                    &flag_id,
                    self.log.logger("flag"),
                    pos,
                    &mut world_pool.flag_pool,
                ),
            );
        }

        fn copy_cores(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.values_mut().for_each(|proj| {
                proj.projectile
                    .copy_core(pipe.next_core_index, pipe.prev_core_index);
                proj.projectile
                    .copy_reusable_core(pipe.next_core_index, pipe.prev_core_index);
            });

            self.flags.values_mut().for_each(|flag| {
                flag.copy_core(pipe.next_core_index, pipe.prev_core_index);
                flag.copy_reusable_core(pipe.next_core_index, pipe.prev_core_index);
            });

            self.pickups.values_mut().for_each(|pickup| {
                pickup.copy_core(pipe.next_core_index, pipe.prev_core_index);
                pickup.copy_reusable_core(pipe.next_core_index, pipe.prev_core_index);
            });

            self.lasers.values_mut().for_each(|laser| {
                laser
                    .laser
                    .copy_core(pipe.next_core_index, pipe.prev_core_index);
                laser
                    .laser
                    .copy_reusable_core(pipe.next_core_index, pipe.prev_core_index);
            });

            pipe.players.values().for_each(|p| {
                self.characters
                    .get_mut(&p.character_info.character_id)
                    .unwrap()
                    .copy_core(pipe.next_core_index, pipe.prev_core_index);
                self.characters
                    .get_mut(&p.character_info.character_id)
                    .unwrap()
                    .copy_reusable_core(pipe.next_core_index, pipe.prev_core_index);
            });
        }

        fn tick_projectiles(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.values_mut().for_each(|proj| {
                Projectile::tick(&mut SimulationPipeProjectile::new(
                    pipe.next_core_index,
                    pipe.collision,
                    &mut proj.projectile,
                    &mut self.characters,
                    pipe.cur_tick,
                    proj.character_id.clone(),
                ));
                // handle the entity events
                let ent = proj.projectile.split_mut(pipe.next_core_index).0;
                let id = ent.game_element_id.clone();
                ent.entity_events.drain(..).for_each(|ev| match ev {
                    EntityEvent::Die { .. } => {
                        self.removed_projectiles_helper.insert(id.clone());
                    }
                    EntityEvent::Projectile { .. } => {
                        todo!()
                    }
                    EntityEvent::Laser { .. } => {
                        todo!()
                    }
                    EntityEvent::Sound { pos, name } => {
                        pipe.simulation_events
                            .push(SimulationEvents::Sound { pos, name });
                    }
                    EntityEvent::Explosion {} => {
                        todo!()
                    }
                });
            });
        }

        fn post_tick_projectiles(&mut self, pipe: &mut SimulationPipeStage) {
            self.projectiles.values_mut().for_each(|proj| {
                Projectile::tick_deferred(&mut SimulationPipeProjectile::new(
                    pipe.next_core_index,
                    pipe.collision,
                    &mut proj.projectile,
                    &mut self.characters,
                    pipe.cur_tick,
                    proj.character_id.clone(),
                ));
            });
        }

        fn tick_flags(&mut self, pipe: &mut SimulationPipeStage) {
            self.flags.values_mut().for_each(|flag| {
                Flag::tick(&mut SimulationPipeFlag::new(pipe.next_core_index, flag));
                // handle the entity events
                let ent = flag.split_mut(pipe.next_core_index).0;
                let id = ent.game_element_id.clone();
                ent.entity_events.drain(..).for_each(|ev| match ev {
                    EntityEvent::Die { .. } => {
                        self.removed_flags_helper.insert(id.clone());
                    }
                    EntityEvent::Projectile { .. } => {
                        todo!()
                    }
                    EntityEvent::Laser { .. } => {
                        todo!()
                    }
                    EntityEvent::Sound { pos, name } => {
                        pipe.simulation_events
                            .push(SimulationEvents::Sound { pos, name });
                    }
                    EntityEvent::Explosion {} => {
                        todo!()
                    }
                });
            });
        }

        fn post_tick_flags(&mut self, pipe: &mut SimulationPipeStage) {
            self.flags.values_mut().for_each(|flag| {
                Flag::tick_deferred(&mut SimulationPipeFlag::new(pipe.next_core_index, flag));
            });
        }

        fn tick_pickups(&mut self, pipe: &mut SimulationPipeStage) {
            self.pickups.values_mut().for_each(|pickup| {
                Pickup::tick(&mut SimulationPipePickup::new(pipe.next_core_index, pickup));
                // handle the entity events
                let ent = pickup.split_mut(pipe.next_core_index).0;
                let id = ent.game_element_id.clone();
                ent.entity_events.drain(..).for_each(|ev| match ev {
                    EntityEvent::Die { .. } => {
                        self.removed_pickups_helper.insert(id.clone());
                    }
                    EntityEvent::Projectile { .. } => {
                        todo!()
                    }
                    EntityEvent::Laser { .. } => {
                        todo!()
                    }
                    EntityEvent::Sound { pos, name } => {
                        pipe.simulation_events
                            .push(SimulationEvents::Sound { pos, name });
                    }
                    EntityEvent::Explosion {} => {
                        todo!()
                    }
                });
            });
        }

        fn post_tick_pickups(&mut self, pipe: &mut SimulationPipeStage) {
            self.pickups.values_mut().for_each(|pickup| {
                Pickup::tick_deferred(&mut SimulationPipePickup::new(pipe.next_core_index, pickup));
            });
        }

        fn tick_lasers(&mut self, pipe: &mut SimulationPipeStage) {
            self.lasers.values_mut().for_each(|laser| {
                Laser::tick(&mut SimulationPipeLaser::new(
                    pipe.next_core_index,
                    &mut laser.laser,
                    pipe.cur_tick,
                    pipe.collision,
                    &mut self.characters,
                    laser.character_id,
                ));
                // handle the entity events
                let ent = laser.laser.split_mut(pipe.next_core_index).0;
                let id = ent.game_element_id.clone();
                ent.entity_events.drain(..).for_each(|ev| match ev {
                    EntityEvent::Die { .. } => {
                        self.removed_lasers_helper.insert(id.clone());
                    }
                    EntityEvent::Projectile { .. } => {
                        todo!()
                    }
                    EntityEvent::Laser { .. } => {
                        todo!()
                    }
                    EntityEvent::Sound { pos, name } => {
                        pipe.simulation_events
                            .push(SimulationEvents::Sound { pos, name });
                    }
                    EntityEvent::Explosion {} => {
                        todo!()
                    }
                });
            });
        }

        fn post_tick_lasers(&mut self, pipe: &mut SimulationPipeStage) {
            self.lasers.values_mut().for_each(|laser| {
                Laser::tick_deferred(&mut SimulationPipeLaser::new(
                    pipe.next_core_index,
                    &mut laser.laser,
                    pipe.cur_tick,
                    pipe.collision,
                    &mut self.characters,
                    laser.character_id,
                ));
            });
        }

        fn tick_characters(&mut self, pipe: &mut SimulationPipeStage) {
            pipe.players.iter().for_each(|(id, p)| {
                Character::tick(&mut SimulationPipeCharacter::new(
                    pipe.next_core_index,
                    p,
                    pipe.players,
                    &mut GetCharacterHelper {
                        characters: &mut self.characters,
                        cur_character_id: &p.character_info.character_id,
                        removed_characters: &self.removed_characters_helper,
                        cur_core_index: pipe.next_core_index,
                    },
                    pipe.collision,
                    pipe.cur_tick,
                ));

                let (ent, _, _) = self
                    .characters
                    .get_mut(&p.character_info.character_id)
                    .unwrap()
                    .split_mut(pipe.next_core_index);
                // handle the entity events
                ent.entity_events.drain(..).for_each(|ev| {
                    match ev {
                        EntityEvent::Die {
                            pos,
                            respawns_at_tick,
                        } => {
                            // player will be removed
                            // if a character dies, assume the player was moved to spec
                            self.removed_players_helper.insert(
                                id.clone(),
                                PlayerRemoveInfo {
                                    respawns_at_tick: respawns_at_tick,
                                    last_stage_id: pipe.stage_id.clone(),
                                    no_char_type: if respawns_at_tick.is_some() {
                                        NoCharPlayerType::Dead
                                    } else {
                                        NoCharPlayerType::Spectator
                                    },
                                    pos,
                                },
                            );
                            self.removed_characters_helper
                                .insert(p.character_info.character_id.clone());
                        }
                        EntityEvent::Projectile { pos, dir, ty } => {
                            let proj_id = pipe.id_generator.get_next();
                            self.projectiles.insert(
                                proj_id.clone(),
                                WorldProjectile {
                                    character_id: ent.game_element_id.clone(),
                                    projectile: Projectile::new(
                                        &proj_id,
                                        self.log.logger("projectile"),
                                        &pos,
                                        &dir,
                                        2 * TICKS_PER_SECOND as i32,
                                        0,
                                        0.0,
                                        pipe.cur_tick,
                                        false,
                                        ty,
                                        &mut pipe.world_pool.projectile_pool,
                                    ),
                                },
                            );
                        }
                        EntityEvent::Laser { pos, dir } => {
                            let id = pipe.id_generator.get_next();
                            self.lasers.insert(
                                id.clone(),
                                WorldLaser {
                                    character_id: ent.game_element_id.clone(),
                                    laser: Laser::new(
                                        &id,
                                        self.log.logger("laser"),
                                        &pos,
                                        &dir,
                                        pipe.cur_tick,
                                        800.0, // TODO:
                                        true,  // TODO:
                                        true,  // TODO:
                                        &mut pipe.world_pool.laser_pool,
                                    ),
                                },
                            );
                        }
                        EntityEvent::Sound { pos, name } => {
                            pipe.simulation_events
                                .push(SimulationEvents::Sound { pos, name });
                        }
                        EntityEvent::Explosion {} => {
                            todo!()
                        }
                    }
                });
            });
        }

        fn post_tick_characters(&mut self, pipe: &mut SimulationPipeStage) {
            pipe.players.values().for_each(|p| {
                Character::tick_deferred(&mut SimulationPipeCharacter::new(
                    pipe.next_core_index,
                    p,
                    pipe.players,
                    &mut GetCharacterHelper {
                        characters: &mut self.characters,
                        cur_character_id: &p.character_info.character_id,
                        removed_characters: &self.removed_characters_helper,
                        cur_core_index: pipe.next_core_index,
                    },
                    pipe.collision,
                    pipe.cur_tick,
                ));
            });
        }

        fn handle_removed_entities(&mut self, pipe: &mut SimulationPipeStage) {
            self.removed_players_helper
                .drain()
                .for_each(|(id, remove_info)| {
                    pipe.simulation_events
                        .push(SimulationEvents::PlayerCharacterRemoved { id, remove_info });
                });
            self.removed_characters_helper.drain().for_each(|id| {
                let core = &mut self
                    .characters
                    .get_mut(&id)
                    .unwrap()
                    .get_reusable_core_at_index_mut(pipe.next_core_index)
                    .core;
                // TODO: swap with pool (or put the ids back to pool)
                let mut attached_character_ids = Default::default();
                std::mem::swap(
                    &mut core.hooked_character.attached_characters_ids,
                    &mut attached_character_ids,
                );

                attached_character_ids.iter().for_each(|attached_char_id| {
                    if let Some(attached_char) = self.characters.get_mut(attached_char_id) {
                        attached_char
                            .get_reusable_core_at_index_mut(pipe.next_core_index)
                            .core
                            .hooked_character
                            .id = None;
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
            if pipe.is_prediction {
                self.copy_cores(pipe);
            }

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
                self.handle_removed_entities(pipe);
            } else {
                self.removed_players_helper.clear();
                self.removed_projectiles_helper.clear();
                self.removed_flags_helper.clear();
                self.removed_pickups_helper.clear();
                self.removed_lasers_helper.clear();
                self.removed_characters_helper.clear();
            }
        }
    }
}
