pub mod pickup {
    use game_interface::types::{game::GameEntityId, pickup::PickupType};
    use hashlink::LinkedHashMap;
    use hiarc::Hiarc;
    use math::math::{lerp, vector::vec2};
    use pool::{datatypes::PoolLinkedHashMap, pool::Pool, recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};
    use shared_base::reusable::{CloneWithCopyableElements, ReusableCore};

    use crate::{
        entities::{
            character::character::CharactersView,
            entity::entity::{Entity, EntityInterface, EntityTickResult},
        },
        events::events::PickupEvent,
        simulation_pipe::simulation_pipe::{
            SimulationEntityEvents, SimulationEventWorldEntityType, SimulationPipePickup,
        },
        weapons::definitions::weapon_def::Weapon,
        world::world::GameWorld,
    };

    #[derive(Debug, Hiarc, Default, Serialize, Deserialize)]
    pub struct PickupReusableCore {}

    impl Recyclable for PickupReusableCore {
        fn new() -> Self {
            Self {}
        }

        fn reset(&mut self) {}
    }

    impl CloneWithCopyableElements for PickupReusableCore {
        fn copy_clone_from(&mut self, _other: &Self) {}
    }

    impl ReusableCore for PickupReusableCore {}

    pub type PoolPickupReusableCore = Recycle<PickupReusableCore>;

    #[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
    pub struct PickupCore {
        pub pos: vec2,
        pub ty: PickupType,
    }

    #[derive(Debug, Hiarc)]
    pub struct Pickup {
        pub(crate) base: Entity,
        pub(crate) core: PickupCore,
        pub(crate) reusable_core: PoolPickupReusableCore,

        simulation_events: SimulationEntityEvents,
    }

    impl Pickup {
        pub fn new(
            game_el_id: &GameEntityId,
            pos: &vec2,
            ty: PickupType,
            pool: &PickupPool,
            simulation_events: &SimulationEntityEvents,
        ) -> Self {
            Self {
                base: Entity::new(game_el_id),
                core: PickupCore { pos: *pos, ty },
                reusable_core: pool.pickup_reusable_cores_pool.new(),

                simulation_events: simulation_events.clone(),
            }
        }

        pub fn lerped_pos(pickup1: &Pickup, pickup2: &Pickup, ratio: f64) -> vec2 {
            lerp(&pickup1.core.pos, &pickup2.core.pos, ratio as f32)
        }
    }

    impl<'a> EntityInterface<PickupCore, PickupReusableCore, SimulationPipePickup<'a>> for Pickup {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipePickup) -> EntityTickResult {
            todo!()
        }

        fn tick(&mut self, pipe: &mut SimulationPipePickup) -> EntityTickResult {
            let intersection = GameWorld::intersect_character(
                pipe.field,
                CharactersView::new(pipe.characters, |_| true),
                &self.core.pos,
                20.0,
            );

            if let Some(char) = intersection {
                // player picked us up, is someone was hooking us, let them go
                // TODO: magic constants
                match self.core.ty {
                    PickupType::PowerupHealth => {
                        if char.core.health < 10 {
                            char.core.health += 1;
                            self.simulation_events.push(
                                Some(char.base.game_element_id),
                                SimulationEventWorldEntityType::Pickup {
                                    id: self.base.game_element_id,
                                    ev: PickupEvent::Pickup {
                                        pos: self.core.pos / 32.0,
                                        ty: PickupType::PowerupHealth,
                                    },
                                },
                            );
                            EntityTickResult::RemoveEntity
                        } else {
                            EntityTickResult::None
                        }
                    }
                    PickupType::PowerupArmor => {
                        if char.core.armor < 10 {
                            char.core.armor += 1;
                            self.simulation_events.push(
                                Some(char.base.game_element_id),
                                SimulationEventWorldEntityType::Pickup {
                                    id: self.base.game_element_id,
                                    ev: PickupEvent::Pickup {
                                        pos: self.core.pos / 32.0,
                                        ty: PickupType::PowerupArmor,
                                    },
                                },
                            );
                            EntityTickResult::RemoveEntity
                        } else {
                            EntityTickResult::None
                        }
                    }
                    PickupType::PowerupWeapon(weapon) => {
                        let res = if let Some(weapon) = char.reusable_core.weapons.get_mut(&weapon)
                        {
                            // check if ammo can be refilled
                            if weapon.cur_ammo.is_some_and(|val| val < 10) {
                                weapon.cur_ammo = Some(10);
                                EntityTickResult::RemoveEntity
                            } else {
                                EntityTickResult::None
                            }
                        }
                        // else add the weapon
                        else {
                            char.reusable_core.weapons.insert(
                                weapon,
                                Weapon {
                                    cur_ammo: Some(10),
                                    next_ammo_regeneration_tick: 0.into(),
                                },
                            );
                            EntityTickResult::RemoveEntity
                        };

                        if res == EntityTickResult::RemoveEntity {
                            self.simulation_events.push(
                                Some(char.base.game_element_id),
                                SimulationEventWorldEntityType::Pickup {
                                    id: self.base.game_element_id,
                                    ev: PickupEvent::Pickup {
                                        pos: self.core.pos / 32.0,
                                        ty: PickupType::PowerupWeapon(weapon),
                                    },
                                },
                            );
                        }
                        res
                    }
                    PickupType::PowerupNinja => {
                        // activate ninja on target player
                        self.simulation_events.push(
                            Some(char.base.game_element_id),
                            SimulationEventWorldEntityType::Pickup {
                                id: self.base.game_element_id,
                                ev: PickupEvent::Pickup {
                                    pos: self.core.pos / 32.0,
                                    ty: PickupType::PowerupNinja,
                                },
                            },
                        );
                        char.give_ninja();
                        EntityTickResult::RemoveEntity
                    }
                }
            } else {
                EntityTickResult::None
            }
        }

        fn tick_deferred(&mut self, _pipe: &mut SimulationPipePickup) -> EntityTickResult {
            EntityTickResult::None
        }

        fn drop_silent(&mut self) {
            self.base.drop_silent = true;
        }
    }

    impl Drop for Pickup {
        fn drop(&mut self) {
            if !self.base.drop_silent {
                self.simulation_events.push(
                    None,
                    SimulationEventWorldEntityType::Pickup {
                        id: self.base.game_element_id,
                        ev: PickupEvent::Despawn {
                            pos: self.core.pos,
                            ty: self.core.ty,
                            respawns_in_ticks: 0.into(),
                        },
                    },
                );
            }
        }
    }

    #[derive(Debug, Hiarc, Clone)]
    pub struct PickupPool {
        pub(crate) pickup_pool: Pool<PoolPickups>,
        pub(crate) pickup_reusable_cores_pool: Pool<PickupReusableCore>,
    }

    pub type PoolPickups = LinkedHashMap<GameEntityId, Pickup>;
    pub type Pickups = PoolLinkedHashMap<GameEntityId, Pickup>;
}
