#[allow(clippy::module_inception)]
pub mod world {
    use std::{num::NonZeroU16, ops::ControlFlow, rc::Rc};

    use base::linked_hash_map_view::{
        LinkedHashMapEntryAndRes, LinkedHashMapExceptView, LinkedHashMapIterExt,
    };
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
                character::{CharacterPlayerTy, CharacterPool, CharactersView},
                core::character_core::{self, Core, CoreReusable},
                hook::character_hook::HookedCharacters,
                player::player::{NoCharPlayers, PoolPlayerInfo},
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
            SimulationEntityEvents, SimulationEventWorldEntity, SimulationEventWorldEntityType,
            SimulationPipeFlag, SimulationPipeLaser, SimulationPipePickup,
            SimulationPipeProjectile,
        },
        spawns::GameSpawns,
        state::state::TICKS_PER_SECOND,
        types::types::GameTeam,
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

                    distance(&other_pos, char_pos) < radius + character_core::PHYSICAL_SIZE
                })
                .for_each(|(_, char)| for_each_func(char, self.removed_characters));
        }

        fn get_other_character_id_and_cores_iter_by_ids_mut(
            &mut self,
            ids: &[GameEntityId],
            for_each_func: &mut dyn FnMut(
                &GameEntityId,
                &mut Core,
                &mut CoreReusable,
                &mut CharacterPos,
            ) -> ControlFlow<()>,
        ) -> ControlFlow<()> {
            ids.iter().try_for_each(|id| {
                if self.removed_characters.is_empty() || !self.removed_characters.contains(id) {
                    if let Some(char) = self.other_characters.get_mut(id) {
                        let (core, reusable_core) = (&mut char.core, &mut char.reusable_core);
                        return for_each_func(
                            id,
                            &mut core.core,
                            &mut reusable_core.core,
                            &mut char.pos,
                        );
                    }
                }
                ControlFlow::Continue(())
            })
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
        pub(crate) removed_characters_helper_pool: Pool<LinkedHashSet<GameEntityId>>,
        pub(crate) projectile_pool: ProjectilePool,
        pub(crate) flag_pool: FlagPool,
        pub(crate) pickup_pool: PickupPool,
        pub(crate) laser_pool: LaserPool,
        pub(crate) character_pool: CharacterPool,
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
        pub(crate) red_flags: Flags,
        pub(crate) blue_flags: Flags,
        pub(crate) pickups: Pickups,
        pub(crate) lasers: Lasers,
        pub(crate) characters: Characters,

        /// inactive / non spawned / whatever game objects
        pub(crate) inactive_game_objects: GameObjectsWorld,

        removed_characters_helper: PoolLinkedHashSet<GameEntityId>,

        pub(crate) world_pool: WorldPool,

        pub(crate) id_generator: Option<IdGenerator>,

        pub simulation_events: SimulationEntityEvents,
        pub(crate) play_field: CharacterPositionPlayfield,
        pub(crate) hooks: HookedCharacters,
    }

    impl GameWorld {
        pub fn new(
            world_pool: &WorldPool,
            game_object_definitions: &Rc<GameObjectDefinitions>,
            width: NonZeroU16,
            height: NonZeroU16,
            id_gen: Option<&IdGenerator>,
        ) -> Self {
            let simulation_events = SimulationEntityEvents::new();
            let mut inactive_game_objects = GameObjectsWorld {
                pickups: Default::default(),
            };

            let mut red_flags = world_pool.flag_pool.flag_pool.new();
            let mut blue_flags = world_pool.flag_pool.flag_pool.new();
            let mut pickups = world_pool.pickup_pool.pickup_pool.new();

            if let Some(id_gen) = id_gen {
                let mut add_pick = |pickup_pos: &ivec2, ty: PickupType| {
                    let id = id_gen.next_id();
                    pickups.insert(
                        id,
                        Pickup::new(
                            &id,
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
                        respawn_in_ticks: TICKS_PER_SECOND * 90,
                    });
                }

                let add_flag = |flags: &mut Flags, pos: &ivec2, ty: FlagType| {
                    let id = id_gen.next_id();
                    flags.insert(
                        id,
                        Flag::new(
                            &id,
                            &(vec2::new(pos.x as f32, pos.y as f32) * 32.0 + vec2::new(16.0, 16.0)),
                            ty,
                            &world_pool.flag_pool,
                            &simulation_events,
                        ),
                    );
                };
                for flag in &game_object_definitions.pickups.red_flags {
                    add_flag(&mut red_flags, flag, FlagType::Red)
                }
                for flag in &game_object_definitions.pickups.blue_flags {
                    add_flag(&mut blue_flags, flag, FlagType::Blue)
                }
            }

            Self {
                removed_characters_helper: world_pool.removed_characters_helper_pool.new(),

                projectiles: world_pool.projectile_pool.projectile_pool.new(),
                red_flags,
                blue_flags,
                pickups,
                lasers: world_pool.laser_pool.laser_pool.new(),
                characters: world_pool.character_pool.character_pool.new(),

                inactive_game_objects,

                world_pool: world_pool.clone(),

                id_generator: id_gen.cloned(),

                simulation_events,
                play_field: CharacterPositionPlayfield::new(width, height),
                hooks: Default::default(),
            }
        }

        pub(crate) fn evaluate_character_team(&self, no_char_players: &NoCharPlayers) -> GameTeam {
            let mut red_team = 0;
            let mut blue_team = 0;
            self.characters.iter().for_each(|(_, char)| {
                match char.core.team {
                    Some(team) => match team {
                        GameTeam::Red => red_team += 1,
                        GameTeam::Blue => blue_team += 1,
                    },
                    None => {
                        // ignore
                    }
                }
            });

            let (dead_red_team, dead_blue_team) = no_char_players.count_players_in_team();
            red_team += dead_red_team;
            blue_team += dead_blue_team;

            if blue_team < red_team {
                GameTeam::Blue
            } else {
                GameTeam::Red
            }
        }

        pub fn add_character(
            &mut self,
            character_id: GameEntityId,
            stage_id: &GameEntityId,
            player_info: PoolPlayerInfo,
            player_input: CharacterInput,
            team: Option<GameTeam>,
            ty: CharacterPlayerTy,
            pos: vec2,
        ) -> &mut Character {
            self.characters.insert(
                character_id,
                Character::new(
                    &character_id,
                    &self.world_pool.character_pool,
                    player_info,
                    player_input,
                    &self.simulation_events,
                    stage_id,
                    ty,
                    pos,
                    &self.play_field,
                    &self.hooks,
                    team,
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

            let ids = field.by_radiusf(pos0, line_len);

            ids.iter().for_each(|id| {
                if let Some(char) = characters.get_mut(id) {
                    let char_pos = *char.pos.pos();
                    let mut intersect_pos = vec2::default();
                    if closest_point_on_line(pos0, pos1, &char_pos, &mut intersect_pos) {
                        let d = distance(&char_pos, &intersect_pos);
                        if d < character_core::PHYSICAL_SIZE + radius {
                            let d = distance(pos0, &intersect_pos);
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
                    characters.into_inner().0.get_mut(id).unwrap(),
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

            let ids = field.by_radiusf(pos, radius);

            ids.iter().for_each(|id| {
                if let Some(char) = characters.get_mut(id) {
                    let char_pos = *char.pos.pos();
                    let d = distance(&char_pos, pos);
                    if d < character_core::PHYSICAL_SIZE + radius && d < closest_distance {
                        closest_distance = d;
                        intersect_char = Some(id);
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
            radius: i32,
        ) -> impl Iterator<Item = &'a mut Character> + 'b
        where
            F: Fn(&GameEntityId) -> bool + 'b,
            'a: 'b,
        {
            let ids = field.by_radius_set(pos, radius);

            let (map, filter) = characters.into_inner();
            let view = CharactersView::new(map, move |id| filter(id) && ids.contains(id));

            view.into_iter().filter_map(move |(_, char)| {
                let char_pos = *char.pos.pos();
                let d = distance(&char_pos, pos);
                (d < character_core::PHYSICAL_SIZE + radius as f32).then_some(char)
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

        pub fn get_red_flags(&self) -> &Flags {
            &self.red_flags
        }

        pub fn get_blue_flags(&self) -> &Flags {
            &self.blue_flags
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
                projectile_id,
                WorldProjectile {
                    character_id: owner_character_id,
                    projectile,
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
                pos,
                dir,
                start_energy,
                can_hit_others,
                can_hit_own,
                &self.world_pool.laser_pool,
                &self.simulation_events,
            );
            self.lasers.insert(
                laser_id,
                WorldLaser {
                    character_id: owner_character_id,
                    laser,
                },
            );
        }

        pub fn insert_new_pickup(&mut self, pickup_id: GameEntityId, pos: &vec2, ty: PickupType) {
            self.pickups.insert(
                pickup_id,
                Pickup::new(
                    &pickup_id,
                    pos,
                    ty,
                    &self.world_pool.pickup_pool,
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
                        proj.character_id,
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
                        proj.character_id,
                        &self.play_field,
                    ))
                    != EntityTickResult::RemoveEntity
            });
        }

        fn tick_flags(
            flags: &mut Flags,
            characters: &mut Characters,
            play_field: &CharacterPositionPlayfield,
            pipe: &mut SimulationPipeStage,
        ) {
            flags.retain_with_order(|_, flag| {
                flag.tick(&mut SimulationPipeFlag::new(
                    pipe.collision,
                    characters,
                    play_field,
                    pipe.is_prediction,
                )) != EntityTickResult::RemoveEntity
            });
        }

        fn post_tick_flags(
            flags: &mut Flags,
            characters: &mut Characters,
            play_field: &CharacterPositionPlayfield,
            pipe: &mut SimulationPipeStage,
        ) {
            flags.retain_with_order(|_, flag| {
                flag.tick_deferred(&mut SimulationPipeFlag::new(
                    pipe.collision,
                    characters,
                    play_field,
                    pipe.is_prediction,
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
                    self.removed_characters_helper.insert(*id);
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
                    self.removed_characters_helper.insert(*id);
                }

                // handle the entity events
                character.entity_events.drain(..).for_each(|ev| {
                    match &ev {
                        CharacterEvent::Despawn { .. } => {
                            self.removed_characters_helper.insert(*id);
                        }
                        CharacterEvent::Projectile {
                            pos,
                            dir,
                            ty,
                            lifetime,
                        } => {
                            if let Some(id_generator) = &self.id_generator {
                                let proj_id = id_generator.next_id();
                                let projectile = Projectile::new(
                                    &proj_id,
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
                                    &pipe.world_pool.projectile_pool,
                                    &self.simulation_events,
                                );
                                self.projectiles.insert(
                                    proj_id,
                                    WorldProjectile {
                                        character_id: character.base.game_element_id,
                                        projectile,
                                    },
                                );
                            }
                        }
                        CharacterEvent::Laser { pos, dir, energy } => {
                            if let Some(id_generator) = &self.id_generator {
                                let id = id_generator.next_id();
                                let laser = Laser::new(
                                    &id,
                                    pos,
                                    dir,
                                    *energy,
                                    true,  // TODO:
                                    false, // TODO:
                                    &pipe.world_pool.laser_pool,
                                    &self.simulation_events,
                                );
                                self.lasers.insert(
                                    id,
                                    WorldLaser {
                                        character_id: character.base.game_element_id,
                                        laser,
                                    },
                                );
                            }
                        }
                        _ => {}
                    }

                    self.simulation_events
                        .push(Some(*id), SimulationEventWorldEntityType::Character { ev });
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
                    self.removed_characters_helper.insert(*id);
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
                self.removed_characters_helper.insert(*id);
            }
        }

        fn handle_removed_entities(&mut self) {
            self.removed_characters_helper.drain().for_each(|id| {
                self.characters.remove(&id);
            });
        }

        pub(crate) fn get_spawn_pos(&self, spawns: &GameSpawns, team: Option<GameTeam>) -> vec2 {
            match team {
                Some(team) => {
                    fn eval_spawn<'a>(
                        characters: &Characters,
                        filter_team: GameTeam,
                        spawn: impl Iterator<Item = &'a vec2>,
                    ) -> vec2 {
                        let max_by = |&spawn1: &&vec2, &spawn2: &&vec2| {
                            let sum_dist_spawn = |spawn: &vec2| {
                                characters
                                    .values()
                                    .map(|char| {
                                        // multiply by factor so that players of the other team
                                        // are considered near.
                                        distance_squared(spawn, char.pos.pos()) as f64
                                            * if char.core.team == Some(filter_team) {
                                                0.5
                                            } else {
                                                1.0
                                            }
                                    })
                                    .min_by(|f1, f2| f1.total_cmp(f2))
                                    .unwrap_or_default()
                            };

                            let dist1 = sum_dist_spawn(spawn1);
                            let dist2 = sum_dist_spawn(spawn2);
                            dist1.total_cmp(&dist2)
                        };
                        spawn.max_by(max_by).cloned().unwrap_or(vec2::default())
                    }
                    match team {
                        GameTeam::Red => eval_spawn(
                            &self.characters,
                            GameTeam::Blue,
                            spawns
                                .spawns_red
                                .iter()
                                .rev()
                                .chain(spawns.spawns.iter().rev()),
                        ),
                        GameTeam::Blue => eval_spawn(
                            &self.characters,
                            GameTeam::Red,
                            spawns
                                .spawns_blue
                                .iter()
                                .rev()
                                .chain(spawns.spawns.iter().rev()),
                        ),
                    }
                }
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
                                    .min_by(|f1, f2| f1.total_cmp(f2))
                                    .unwrap_or_default()
                            };

                            sum_dist_spawn(spawn1).total_cmp(&sum_dist_spawn(spawn2))
                        })
                        .cloned()
                        .unwrap_or(vec2::default())
                }
            }
        }

        fn handle_simulation_events(&mut self, events: &[SimulationEventWorldEntity]) {
            for SimulationEventWorldEntity { ev, .. } in events.iter() {
                match ev {
                    SimulationEventWorldEntityType::Character { .. }
                    | SimulationEventWorldEntityType::Projectile { .. }
                    | SimulationEventWorldEntityType::Laser { .. } => {
                        // ignore
                    }
                    SimulationEventWorldEntityType::Pickup { ev, .. } => match ev {
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
                    SimulationEventWorldEntityType::Flag { ev, .. } => match ev {
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
                        FlagEvent::Sound { .. }
                        | FlagEvent::Effect { .. }
                        | FlagEvent::Capture { .. } => {
                            // ignore
                        }
                    },
                }
            }
        }

        fn check_inactive_game_objects(&mut self) {
            if let Some(id_generator) = &self.id_generator {
                let mut add_pickup = |obj: &mut GameObjectWorld, ty: PickupType| {
                    obj.respawn_in_ticks -= 1;
                    if obj.respawn_in_ticks == 0 {
                        let pos = vec2::new(obj.pos.x as f32, obj.pos.y as f32) * 32.0
                            + vec2::new(16.0, 16.0);
                        let id = id_generator.next_id();
                        self.pickups.insert(
                            id,
                            Pickup::new(
                                &id,
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
        }

        #[must_use]
        pub fn tick(
            &mut self,
            pipe: &mut SimulationPipeStage,
        ) -> PoolVec<SimulationEventWorldEntity> {
            self.check_inactive_game_objects();

            self.tick_characters(pipe);
            self.tick_projectiles(pipe);
            Self::tick_flags(
                &mut self.red_flags,
                &mut self.characters,
                &self.play_field,
                pipe,
            );
            Self::tick_flags(
                &mut self.blue_flags,
                &mut self.characters,
                &self.play_field,
                pipe,
            );
            self.tick_pickups(pipe);
            self.tick_lasers(pipe);

            self.post_tick_characters(pipe);
            self.post_tick_projectiles(pipe);
            Self::post_tick_flags(
                &mut self.red_flags,
                &mut self.characters,
                &self.play_field,
                pipe,
            );
            Self::post_tick_flags(
                &mut self.blue_flags,
                &mut self.characters,
                &self.play_field,
                pipe,
            );
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
