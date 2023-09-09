pub mod pickup {
    use base_log::log::SystemLogGroup;
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::{lerp, vector::vec2};
    use pool::{datatypes::PoolLinkedHashMap, pool::Pool, recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};
    use shared_base::{
        game_types::TGameElementID,
        reuseable::{CloneWithCopyableElements, ReusableCore},
    };

    use crate::{
        entities::entity::entity::{Entity, EntityInterface},
        simulation_pipe::simulation_pipe::SimulationPipePickup,
        weapons::definitions::weapon_def::WeaponType,
    };

    #[derive(Default, Serialize, Deserialize, Encode, Decode)]
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

    #[derive(Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub enum PickupType {
        #[default]
        PowerupHealth,
        PowerupArmor,
        // TODO: PowerupNinja,
    }

    #[derive(Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct PickupCore {
        pub pos: vec2,
        pub ty: PickupType,
        pub weapon: WeaponType,
    }

    pub struct Pickup {
        base: Entity,
        cores: [PickupCore; 2],
        reusable_cores: [PoolPickupReusableCore; 2],
    }

    impl Pickup {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            pool: &mut PickupPool,
        ) -> Self {
            Self {
                base: Entity::new(game_el_id, logger),
                cores: [
                    PickupCore {
                        pos: *pos,
                        ty: PickupType::PowerupHealth,
                        weapon: WeaponType::Gun,
                    },
                    PickupCore {
                        pos: *pos,
                        ty: PickupType::PowerupHealth,
                        weapon: WeaponType::Gun,
                    },
                ],
                reusable_cores: [
                    pool.pickup_reusable_cores_pool.new(),
                    pool.pickup_reusable_cores_pool.new(),
                ],
            }
        }

        pub fn lerped_pos(&self, ratio: f64) -> vec2 {
            lerp(&self.cores[0].pos, &self.cores[1].pos, ratio as f32)
        }
    }

    pub struct PickupPool {
        pub(crate) pickup_pool: Pool<PoolPickups>,
        pub(crate) pickup_reusable_cores_pool: Pool<PickupReusableCore>,
    }

    impl<'a> EntityInterface<PickupCore, PickupReusableCore, SimulationPipePickup<'a>> for Pickup {
        fn pre_tick(_pipe: &mut SimulationPipePickup) {
            todo!()
        }

        fn tick(_pipe: &mut SimulationPipePickup) {}

        fn tick_deferred(_pipe: &mut SimulationPipePickup) {}

        fn split_mut(
            &mut self,
            index: usize,
        ) -> (
            &mut Entity,
            &mut PickupCore,
            &mut Recycle<PickupReusableCore>,
        ) {
            (
                &mut self.base,
                &mut self.cores[index],
                &mut self.reusable_cores[index],
            )
        }

        fn get_core_at_index(&self, index: usize) -> &PickupCore {
            &self.cores[index]
        }

        fn get_core_at_index_mut(&mut self, index: usize) -> &mut PickupCore {
            &mut self.cores[index]
        }

        fn get_reusable_cores_mut(&mut self) -> &mut [pool::recycle::Recycle<PickupReusableCore>] {
            &mut self.reusable_cores
        }

        fn get_reusable_core_at_index(
            &self,
            index: usize,
        ) -> &pool::recycle::Recycle<PickupReusableCore> {
            &self.reusable_cores[index]
        }

        fn get_reusable_core_at_index_mut(
            &mut self,
            index: usize,
        ) -> &mut pool::recycle::Recycle<PickupReusableCore> {
            &mut self.reusable_cores[index]
        }
    }

    pub type PoolPickups = LinkedHashMap<TGameElementID, Pickup>;
    pub type Pickups = PoolLinkedHashMap<TGameElementID, Pickup>;

    #[derive(Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct PickupRenderInfo {
        pub ty: PickupType,
        pub weapon: WeaponType,
        pub pos: vec2,
    }
}
