pub mod world {
    use hashlink::{LinkedHashMap, LinkedHashSet};
    use math::math::{closest_point_on_line, distance, vector::vec2};
    use pool::{
        datatypes::{PoolLinkedHashMap, PoolLinkedHashSet},
        pool::Pool,
    };

    use shared_base::{game_types::TGameElementID, id_gen::IDGenerator};

    use crate::{
        entities::{
            character::character::{self, CharacterPool},
            entity::entity::{EntitiyEvent, EntityInterface},
            projectile::projectile::{Projectile, ProjectilePool, WorldProjectile},
        },
        simulation_pipe::simulation_pipe::SimulationPipeProjectile,
    };

    use super::super::{
        entities::{
            character::character::{Character, Characters},
            character_core::character_core::{Core, CoreReusable},
            projectile::projectile::{PoolProjectiles, Projectiles},
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
        character_pool: CharacterPool,
        removed_players_helper_pool: Pool<LinkedHashMap<TGameElementID, PlayerRemoveInfo>>,
        removed_characters_helper_pool: Pool<LinkedHashSet<TGameElementID>>,
        projectile_pool: ProjectilePool,
    }

    impl WorldPool {
        pub fn new(max_characters: usize) -> Self {
            Self {
                character_pool: CharacterPool {
                    character_pool: Pool::with_capacity(max_characters),
                    // reusable cores are used in snapshots quite frequently, and thus worth being pooled
                    // multiply by 2, because every character has two cores of this type
                    character_reusable_cores_pool: Pool::with_capacity(max_characters * 2),
                },
                removed_players_helper_pool: Pool::with_capacity(max_characters),
                removed_characters_helper_pool: Pool::with_capacity(max_characters),
                projectile_pool: ProjectilePool {
                    projectile_pool: Pool::with_capacity(1024), // TODO: add hint for this
                    projectile_reusable_cores_pool: Pool::with_capacity(1024 * 2), // TODO: add hint for this
                },
            }
        }
    }

    pub struct GameWorld {
        /*
            ENTTYPE_PROJECTILE = 0,
            ENTTYPE_LASER,
            ENTTYPE_PICKUP,
            ENTTYPE_CHARACTER,
            ENTTYPE_FLAG,
            NUM_ENTTYPES
        */
        pub(crate) projectiles: Projectiles,
        characters: Characters,

        removed_players_helper: PoolLinkedHashMap<TGameElementID, PlayerRemoveInfo>,
        removed_characters_helper: PoolLinkedHashSet<TGameElementID>,
    }

    impl GameWorld {
        pub fn new(world_pool: &mut WorldPool) -> Self {
            Self {
                characters: world_pool.character_pool.character_pool.new(),
                removed_players_helper: world_pool.removed_players_helper_pool.new(),
                removed_characters_helper: world_pool.removed_characters_helper_pool.new(),
                projectiles: world_pool.projectile_pool.projectile_pool.new(),
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
                Character::new(&id, &mut characters_pool.character_pool),
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
                Character::new(&character_id, &mut characters_pool.character_pool),
            );
        }

        pub fn rem_character(&mut self, char_id: &TGameElementID) -> Option<Character> {
            self.characters.remove(char_id)
        }

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

        pub fn tick(&mut self, pipe: &mut SimulationPipeStage) {
            if pipe.is_prediction {
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

                self.projectiles.values_mut().for_each(|proj| {
                    proj.projectile
                        .copy_core(pipe.next_core_index, pipe.prev_core_index);
                    proj.projectile
                        .copy_reusable_core(pipe.next_core_index, pipe.prev_core_index);
                });
            }

            self.removed_players_helper.clear();

            self.projectiles.values_mut().for_each(|proj| {
                Projectile::tick(&mut SimulationPipeProjectile::new(
                    pipe.next_core_index,
                    pipe.collision,
                    &mut proj.projectile,
                    &mut self.characters,
                    pipe.cur_tick,
                    proj.character_id.clone(),
                ));
            });
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
                        EntitiyEvent::Die {
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
                        EntitiyEvent::Projectile { pos, dir } => {
                            let proj_id = pipe.id_generator.get_next();
                            self.projectiles.insert(
                                proj_id.clone(),
                                WorldProjectile {
                                    character_id: ent.game_element_id.clone(),
                                    projectile: Projectile::new(
                                        &proj_id,
                                        &pos,
                                        &dir,
                                        0,
                                        0,
                                        0.0,
                                        pipe.cur_tick,
                                        false,
                                        &mut pipe.world_pool.projectile_pool,
                                    ),
                                },
                            );
                        }
                        EntitiyEvent::Sound {} => {
                            todo!()
                        }
                        EntitiyEvent::Explosion {} => {
                            todo!()
                        }
                    }
                });
            });

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

            if !pipe.is_prediction {
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
            } else {
                self.removed_players_helper.clear();
                self.removed_characters_helper.clear();
            }
        }
    }
}
