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

    pub struct Flag {
        pub(crate) base: Entity,
        cores: [FlagCore; 2],
        reusable_cores: [PoolFlagReusableCore; 2],
    }

    impl Flag {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            pool: &mut FlagPool,
        ) -> Self {
            Self {
                base: Entity::new(game_el_id, logger),
                cores: [
                    FlagCore {
                        pos: *pos,
                        ty: FlagType::Red,
                    },
                    FlagCore {
                        pos: *pos,
                        ty: FlagType::Red,
                    },
                ],
                reusable_cores: [
                    pool.flag_reusable_cores_pool.new(),
                    pool.flag_reusable_cores_pool.new(),
                ],
            }
        }

        pub fn lerped_pos(&self, ratio: f64) -> vec2 {
            lerp(&self.cores[0].pos, &self.cores[1].pos, ratio as f32)
        }
    }

    pub struct FlagPool {
        pub(crate) flag_pool: Pool<PoolFlags>,
        pub(crate) flag_reusable_cores_pool: Pool<FlagReusableCore>,
    }

    impl<'a> EntityInterface<FlagCore, FlagReusableCore, SimulationPipeFlag<'a>> for Flag {
        fn pre_tick(_pipe: &mut SimulationPipeFlag) {
            todo!()
        }

        fn tick(_pipe: &mut SimulationPipeFlag) {
            todo!()
        }

        fn tick_deferred(_pipe: &mut SimulationPipeFlag) {
            todo!()
        }

        fn split_mut(
            &mut self,
            index: usize,
        ) -> (&mut Entity, &mut FlagCore, &mut Recycle<FlagReusableCore>) {
            (
                &mut self.base,
                &mut self.cores[index],
                &mut self.reusable_cores[index],
            )
        }

        fn get_core_at_index(&self, index: usize) -> &FlagCore {
            &self.cores[index]
        }

        fn get_core_at_index_mut(&mut self, index: usize) -> &mut FlagCore {
            &mut self.cores[index]
        }

        fn get_reusable_cores_mut(&mut self) -> &mut [pool::recycle::Recycle<FlagReusableCore>] {
            &mut self.reusable_cores
        }

        fn get_reusable_core_at_index(
            &self,
            index: usize,
        ) -> &pool::recycle::Recycle<FlagReusableCore> {
            &self.reusable_cores[index]
        }

        fn get_reusable_core_at_index_mut(
            &mut self,
            index: usize,
        ) -> &mut pool::recycle::Recycle<FlagReusableCore> {
            &mut self.reusable_cores[index]
        }
    }

    pub type PoolFlags = LinkedHashMap<TGameElementID, Flag>;
    pub type Flags = PoolLinkedHashMap<TGameElementID, Flag>;

    #[derive(Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct FlagRenderInfo {
        pub pos: vec2,
    }
}
