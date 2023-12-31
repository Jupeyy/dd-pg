pub mod pickup {
    use base_log::log::SystemLogGroup;
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::{lerp, vector::vec2};
    use pool::{
        datatypes::PoolLinkedHashMap, mt_recycle::Recycle as MtRecycle, pool::Pool,
        recycle::Recycle, traits::Recyclable,
    };
    use serde::{Deserialize, Serialize};
    use shared_base::{
        game_types::TGameElementID,
        network::messages::WeaponType,
        reuseable::{CloneWithCopyableElements, ReusableCore},
    };

    use crate::{
        entities::entity::entity::{Entity, EntityInterface},
        events::events::PickupEvent,
        simulation_pipe::simulation_pipe::SimulationPipePickup,
    };

    #[derive(Debug, Default, Serialize, Deserialize, Encode, Decode)]
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
    pub type MtPoolPickupReusableCore = MtRecycle<PickupReusableCore>;

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub enum PickupType {
        #[default]
        PowerupHealth,
        PowerupArmor,
        // TODO: PowerupNinja,
    }

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct PickupCore {
        pub pos: vec2,
        pub ty: PickupType,
        pub weapon: WeaponType,
    }

    #[derive(Debug)]
    pub struct Pickup {
        pub(crate) base: Entity,
        core: PickupCore,
        reusable_core: PoolPickupReusableCore,

        pub(crate) entity_events: Vec<PickupEvent>,
    }

    impl Pickup {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            pool: &PickupPool,
        ) -> Self {
            Self {
                base: Entity::new(game_el_id, logger),
                core: PickupCore {
                    pos: *pos,
                    ty: PickupType::PowerupHealth,
                    weapon: WeaponType::Gun,
                },
                reusable_core: pool.pickup_reusable_cores_pool.new(),

                entity_events: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct PickupPool {
        pub(crate) pickup_pool: Pool<PoolPickups>,
        pub(crate) pickup_reusable_cores_pool: Pool<PickupReusableCore>,
    }

    impl<'a> EntityInterface<PickupCore, PickupReusableCore, SimulationPipePickup> for Pickup {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipePickup) {
            todo!()
        }

        fn tick(&mut self, _pipe: &mut SimulationPipePickup) {}

        fn tick_deferred(&mut self, _pipe: &mut SimulationPipePickup) {}

        fn split(&self) -> (&Entity, &PickupCore, &Recycle<PickupReusableCore>) {
            (&self.base, &self.core, &self.reusable_core)
        }

        fn split_mut(
            &mut self,
        ) -> (
            &mut Entity,
            &mut PickupCore,
            &mut Recycle<PickupReusableCore>,
        ) {
            (&mut self.base, &mut self.core, &mut self.reusable_core)
        }
    }

    pub fn lerped_pos(pickup1: &Pickup, pickup2: &Pickup, ratio: f64) -> vec2 {
        lerp(&pickup1.core.pos, &pickup2.core.pos, ratio as f32)
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
