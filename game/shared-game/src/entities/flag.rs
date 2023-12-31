pub mod flag {
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
        reuseable::{CloneWithCopyableElements, ReusableCore},
    };

    use crate::{
        entities::entity::entity::{Entity, EntityInterface},
        events::events::FlagEvent,
        simulation_pipe::simulation_pipe::SimulationPipeFlag,
    };

    #[derive(Debug, Default, Serialize, Deserialize, Encode, Decode)]
    pub struct FlagReusableCore {}

    impl Recyclable for FlagReusableCore {
        fn new() -> Self {
            Self {}
        }

        fn reset(&mut self) {}
    }

    impl CloneWithCopyableElements for FlagReusableCore {
        fn copy_clone_from(&mut self, _other: &Self) {}
    }

    impl ReusableCore for FlagReusableCore {}

    pub type PoolFlagReusableCore = Recycle<FlagReusableCore>;
    pub type MtPoolFlagReusableCore = MtRecycle<FlagReusableCore>;

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub enum FlagType {
        #[default]
        Red,
        Blue,
    }

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct FlagCore {
        pub pos: vec2,
        pub ty: FlagType,
    }

    #[derive(Debug)]
    pub struct Flag {
        pub(crate) base: Entity,
        core: FlagCore,
        reusable_core: PoolFlagReusableCore,

        pub(crate) entity_events: Vec<FlagEvent>,
    }

    impl Flag {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            pool: &FlagPool,
        ) -> Self {
            Self {
                base: Entity::new(game_el_id, logger),
                core: FlagCore {
                    pos: *pos,
                    ty: FlagType::Red,
                },
                reusable_core: pool.flag_reusable_cores_pool.new(),

                entity_events: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct FlagPool {
        pub(crate) flag_pool: Pool<PoolFlags>,
        pub(crate) flag_reusable_cores_pool: Pool<FlagReusableCore>,
    }

    impl<'a> EntityInterface<FlagCore, FlagReusableCore, SimulationPipeFlag> for Flag {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipeFlag) {
            todo!()
        }

        fn tick(&mut self, _pipe: &mut SimulationPipeFlag) {
            todo!()
        }

        fn tick_deferred(&mut self, _pipe: &mut SimulationPipeFlag) {
            todo!()
        }

        fn split(&self) -> (&Entity, &FlagCore, &Recycle<FlagReusableCore>) {
            (&self.base, &self.core, &self.reusable_core)
        }

        fn split_mut(&mut self) -> (&mut Entity, &mut FlagCore, &mut Recycle<FlagReusableCore>) {
            (&mut self.base, &mut self.core, &mut self.reusable_core)
        }
    }

    pub fn lerped_pos(flag1: &Flag, flag2: &Flag, ratio: f64) -> vec2 {
        lerp(&flag1.core.pos, &flag2.core.pos, ratio as f32)
    }

    pub type PoolFlags = LinkedHashMap<TGameElementID, Flag>;
    pub type Flags = PoolLinkedHashMap<TGameElementID, Flag>;

    #[derive(Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct FlagRenderInfo {
        pub pos: vec2,
    }
}
