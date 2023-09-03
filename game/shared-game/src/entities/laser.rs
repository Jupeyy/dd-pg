pub mod laser {
    use base_log::log::SystemLogGroup;
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::vector::vec2;
    use pool::datatypes::PoolLinkedHashMap;
    use pool::pool::Pool;
    use pool::{recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};

    use shared_base::game_types::TGameElementID;
    use shared_base::{
        reuseable::{CloneWithCopyableElements, ReusableCore},
        types::GameTickType,
    };

    use crate::entities::entity::entity::{Entity, EntityInterface};
    use crate::simulation_pipe::simulation_pipe::SimulationPipeLaser;

    #[derive(Default, Serialize, Deserialize, Encode, Decode)]
    pub struct LaserReusableCore {}

    impl Recyclable for LaserReusableCore {
        fn new() -> Self {
            Self {}
        }

        fn reset(&mut self) {}
    }

    impl CloneWithCopyableElements for LaserReusableCore {
        fn copy_clone_from(&mut self, _other: &Self) {}
    }

    impl ReusableCore for LaserReusableCore {}

    pub type PoolLaserReusableCore = Recycle<LaserReusableCore>;

    #[derive(Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub enum LaserType {
        #[default]
        Rifle,
        Shotgun, // TODO: rename to puller
        Door,
        Freeze,
    }

    #[derive(Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct LaserCore {
        pub pos: vec2,
        pub from: vec2,
        pub dir: vec2,
        pub ty: LaserType,
        pub start_tick: GameTickType,

        pub energy: f32,
        pub bounces: usize,
        pub eval_tick: GameTickType,
        // TODO: int m_Owner;
        // TODO: int m_TeamMask;
        // TODO: bool m_ZeroEnergyBounceInLastTick;

        // DDRace

        /*vec2 m_TelePos;
        bool m_WasTele;
        vec2 m_PrevPos;
        int m_Type;
        int m_TuneZone;
        bool m_TeleportCancelled;
        bool m_IsBlueTeleport;
        bool m_BelongsToPracticeTeam;*/
    }

    pub struct LaserPool {
        pub(crate) _laser_pool: Pool<PoolLasers>,
        pub(crate) laser_reusable_cores_pool: Pool<LaserReusableCore>,
    }

    pub struct Laser {
        base: Entity,
        cores: [LaserCore; 2],
        reusable_cores: [PoolLaserReusableCore; 2],
    }

    impl Laser {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            dir: &vec2,
            start_tick: GameTickType,
            start_energy: f32,
            pool: &mut LaserPool,
        ) -> Self {
            Self {
                base: Entity::new(game_el_id, logger),
                cores: [
                    LaserCore {
                        pos: *pos,
                        from: *pos,
                        start_tick,
                        ty: LaserType::Rifle,
                        bounces: 0,
                        dir: *dir,
                        energy: start_energy,
                        eval_tick: start_tick,
                    },
                    LaserCore {
                        pos: *pos,
                        from: *pos,
                        start_tick,
                        ty: LaserType::Rifle,
                        bounces: 0,
                        dir: *dir,
                        energy: start_energy,
                        eval_tick: start_tick,
                    },
                ],
                reusable_cores: [
                    pool.laser_reusable_cores_pool.new(),
                    pool.laser_reusable_cores_pool.new(),
                ],
            }
        }

        /*
        Mhh: dont like
        void CLaser::LoseOwner()
        {
            if(m_OwnerTeam == TEAM_BLUE)
                m_Owner = PLAYER_TEAM_BLUE;
            else
                m_Owner = PLAYER_TEAM_RED;
        }

        void CLaser::FillInfo(CNetObj_Laser *pProj)
        {
            pProj->m_X = round_to_int(m_Pos.x);
            pProj->m_Y = round_to_int(m_Pos.y);
            pProj->m_VelX = round_to_int(m_Direction.x*100.0f);
            pProj->m_VelY = round_to_int(m_Direction.y*100.0f);
            pProj->m_StartTick = m_StartTick;
            pProj->m_Type = m_Type;
        }*/
    }

    impl<'a> EntityInterface<LaserCore, LaserReusableCore, SimulationPipeLaser<'a>> for Laser {
        fn pre_tick(_pipe: &mut SimulationPipeLaser) {
            todo!()
        }

        fn tick(_pipe: &mut SimulationPipeLaser) {
            todo!()
        }

        fn tick_deferred(_pipe: &mut SimulationPipeLaser) {
            // TODO: todo!()
        }

        fn split_mut(
            &mut self,
            index: usize,
        ) -> (&mut Entity, &mut LaserCore, &mut Recycle<LaserReusableCore>) {
            (
                &mut self.base,
                &mut self.cores[index],
                &mut self.reusable_cores[index],
            )
        }

        fn get_core_at_index(&self, index: usize) -> &LaserCore {
            &self.cores[index]
        }

        fn get_core_at_index_mut(&mut self, index: usize) -> &mut LaserCore {
            &mut self.cores[index]
        }

        fn get_reusable_cores_mut(&mut self) -> &mut [pool::recycle::Recycle<LaserReusableCore>] {
            &mut self.reusable_cores
        }

        fn get_reusable_core_at_index(
            &self,
            index: usize,
        ) -> &pool::recycle::Recycle<LaserReusableCore> {
            &self.reusable_cores[index]
        }

        fn get_reusable_core_at_index_mut(
            &mut self,
            index: usize,
        ) -> &mut pool::recycle::Recycle<LaserReusableCore> {
            &mut self.reusable_cores[index]
        }
    }

    pub struct WorldLaser {
        pub character_id: TGameElementID,
        pub laser: Laser,
    }

    pub type PoolLasers = LinkedHashMap<TGameElementID, WorldLaser>;
    pub type Lasers = PoolLinkedHashMap<TGameElementID, WorldLaser>;

    #[derive(Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct LaserRenderInfo {
        pub ty: LaserType,
        pub from: vec2,
        pub pos: vec2,
        pub start_tick: GameTickType,
    }
}
