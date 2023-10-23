pub mod projectile {
    use base_log::log::SystemLogGroup;
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::lerp;
    use math::math::vector::vec2;
    use pool::datatypes::PoolLinkedHashMap;
    use pool::pool::Pool;
    use pool::{mt_recycle::Recycle as MtRecycle, recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};

    use shared_base::game_types::TGameElementID;
    use shared_base::network::messages::WeaponType;
    use shared_base::{
        reuseable::{CloneWithCopyableElements, ReusableCore},
        types::GameTickType,
    };

    use crate::entities::character::character::{Character, DamageTypes};
    use crate::entities::entity::entity::{calc_pos_and_vel, Entity, EntityInterface};
    use crate::events::events::EntityEvent;
    use crate::simulation_pipe::simulation_pipe::SimulationPipeProjectile;
    use crate::state::state::TICKS_PER_SECOND;
    use crate::world::world::GameWorld;

    #[derive(Debug, Default, Serialize, Deserialize, Encode, Decode)]
    pub struct ProjectileReusableCore {}

    impl Recyclable for ProjectileReusableCore {
        fn new() -> Self {
            Self {}
        }

        fn reset(&mut self) {}
    }

    impl CloneWithCopyableElements for ProjectileReusableCore {
        fn copy_clone_from(&mut self, _other: &Self) {}
    }

    impl ReusableCore for ProjectileReusableCore {}

    pub type PoolProjectileReusableCore = Recycle<ProjectileReusableCore>;
    pub type MtPoolProjectileReusableCore = MtRecycle<ProjectileReusableCore>;

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct ProjectileCore {
        pub pos: vec2,
        pub vel: vec2,
        pub direction: vec2,
        pub life_span: i32,
        // TODO: int m_Owner;
        // TODO: int m_Type;
        pub damage: u32,
        // TODO: int m_SoundImpact;
        pub force: f32,
        pub start_tick: GameTickType,
        pub is_explosive: bool,
        pub ty: WeaponType,
        // DDRace
        // TODO: int m_Bouncing;
        // TODO: bool m_Freeze;
        // TODO: int m_TuneZone;
        // TODO: bool m_BelongsToPracticeTeam;

        // TODO: m_Owner = Owner;
        // TODO: m_OwnerTeam = GameServer()->m_apPlayers[Owner]->GetTeam();
        // TODO: m_Damage = Damage;
        // TODO: m_SoundImpact = SoundImpact;
    }

    pub struct ProjectilePool {
        pub(crate) projectile_pool: Pool<PoolProjectiles>,
        pub(crate) projectile_reusable_cores_pool: Pool<ProjectileReusableCore>,
    }

    pub struct Projectile {
        pub(crate) base: Entity,
        cores: [ProjectileCore; 2],
        reusable_cores: [PoolProjectileReusableCore; 2],
    }

    impl Projectile {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            direction: &vec2,
            life_span: i32,
            damage: u32,
            force: f32,
            start_tick: GameTickType,
            explosive: bool,
            ty: WeaponType,
            pool: &mut ProjectilePool,
        ) -> Self {
            let core = ProjectileCore {
                pos: *pos,
                vel: *direction,
                direction: *direction,
                life_span,
                damage,
                force,
                start_tick,
                is_explosive: explosive,
                ty,
            };
            Self {
                base: Entity::new(game_el_id, logger),
                cores: [core, core],
                reusable_cores: [
                    pool.projectile_reusable_cores_pool.new(),
                    pool.projectile_reusable_cores_pool.new(),
                ],
            }
        }

        pub fn lerped_pos(&self, ratio: f64) -> vec2 {
            lerp(&self.cores[0].pos, &self.cores[1].pos, ratio as f32)
        }
        pub fn estimated_fly_direction(&self, ratio: f64) -> vec2 {
            lerp(&self.cores[0].vel, &self.cores[1].vel, ratio as f32)
        }

        fn advance_pos_and_dir(core: &mut ProjectileCore, pos: &mut vec2, time: f32) {
            let curvature; // TODO
            let speed; // TODO

            match core.ty {
                WeaponType::Grenade => {
                    curvature = 7.0; // TODO: tuning.m_GrenadeCurvature;
                    speed = 1000.0; // TODO: tuning.m_GrenadeSpeed;
                }
                WeaponType::Shotgun => {
                    curvature = 0.0; // TODO: tuning.m_ShotgunCurvature;
                    speed = 0.0; // TODO: tuning.m_ShotgunSpeed;
                }
                WeaponType::Gun => {
                    curvature = 0.0; // TODO: tuning.m_GunCurvature;
                    speed = 1400.0; // TODO: tuning.m_GunSpeed;
                }
                _ => panic!("Weapon types other than grenade, shotgun or gun are not supported"),
            }

            calc_pos_and_vel(pos, &mut core.vel, curvature, speed, time)
        }

        /*
                Mhh: dont like
                void CProjectile::LoseOwner()
                {
                    if(m_OwnerTeam == TEAM_BLUE)
                        m_Owner = PLAYER_TEAM_BLUE;
                    else
                        m_Owner = PLAYER_TEAM_RED;
                }
        */
    }

    impl<'a> EntityInterface<ProjectileCore, ProjectileReusableCore, SimulationPipeProjectile<'a>>
        for Projectile
    {
        fn pre_tick(_pipe: &mut SimulationPipeProjectile) {
            todo!()
        }

        fn tick(pipe: &mut SimulationPipeProjectile) {
            let (ent, core, _) = pipe.projectile.get_ent_and_core_mut();
            let ticks_per_second = TICKS_PER_SECOND;
            let prev_pos = core.pos; // Self::get_pos(core, pt);
            let mut cur_pos = core.pos;
            Self::advance_pos_and_dir(core, &mut cur_pos, 1.0 / (ticks_per_second as f32));
            let mut dummy_pos = Default::default();
            let mut dummy_tele_nr = Default::default();
            let collide = pipe.collision.intersect_line_tele_hook(
                &prev_pos,
                &cur_pos.clone(),
                &mut cur_pos,
                &mut dummy_pos,
                &mut dummy_tele_nr,
            );

            core.life_span -= 1;

            let core_index = pipe.cur_core_index;

            //CCharacter *OwnerChar = GameServer()->GetPlayerChar(m_Owner);
            //CCharacter *TargetChr = GameWorld()->IntersectCharacter(prev_pos, cur_pos, 6.0f, cur_pos, OwnerChar);
            let intersection = GameWorld::intersect_character(
                pipe.characters_helper.get_characters_except_owner(),
                core_index,
                &prev_pos,
                &cur_pos,
                6.0,
            );

            if intersection.is_some()
                || collide > 0
                || core.life_span < 0
                || Entity::outside_of_playfield(&cur_pos, pipe.collision)
            {
                if core.life_span >= 0 || core.ty == WeaponType::Grenade {
                    //ent.entity_events.push(EntitiyEvent::Sound {}); // TODO: GameServer()->CreateSound(cur_pos, m_SoundImpact);
                }

                if core.is_explosive {
                    ent.entity_events.push(EntityEvent::Explosion {}); // TODO: GameServer()->CreateExplosion(cur_pos, m_Owner, m_Weapon, m_Damage);
                } else if let Some((_, _, intersect_char)) = intersection {
                    let intersect_char_id = intersect_char.base.game_element_id.clone();
                    Character::take_damage(
                        pipe.characters_helper.characters,
                        &intersect_char_id,
                        core_index,
                        pipe.cur_tick,
                        &(core.vel * 0.001_f32.max(core.force)),
                        &(core.vel * -1.0),
                        core.damage,
                        DamageTypes::Character(&pipe.characters_helper.owner_character),
                        core.ty,
                    );
                }
                ent.entity_events.push(EntityEvent::Die {
                    pos: core.pos,
                    respawns_at_tick: None,
                });
            }
            core.pos = cur_pos;
        }

        fn tick_deferred(_pipe: &mut SimulationPipeProjectile) {
            // TODO: todo!()
        }

        fn split_mut(
            &mut self,
            index: usize,
        ) -> (
            &mut Entity,
            &mut ProjectileCore,
            &mut Recycle<ProjectileReusableCore>,
        ) {
            (
                &mut self.base,
                &mut self.cores[index],
                &mut self.reusable_cores[index],
            )
        }

        fn get_core_at_index(&self, index: usize) -> &ProjectileCore {
            &self.cores[index]
        }

        fn get_core_at_index_mut(&mut self, index: usize) -> &mut ProjectileCore {
            &mut self.cores[index]
        }

        fn get_reusable_cores_mut(
            &mut self,
        ) -> &mut [pool::recycle::Recycle<ProjectileReusableCore>] {
            &mut self.reusable_cores
        }

        fn get_reusable_core_at_index(
            &self,
            index: usize,
        ) -> &pool::recycle::Recycle<ProjectileReusableCore> {
            &self.reusable_cores[index]
        }

        fn get_reusable_core_at_index_mut(
            &mut self,
            index: usize,
        ) -> &mut pool::recycle::Recycle<ProjectileReusableCore> {
            &mut self.reusable_cores[index]
        }
    }

    pub struct WorldProjectile {
        pub character_id: TGameElementID,
        pub projectile: Projectile,
    }

    pub type PoolProjectiles = LinkedHashMap<TGameElementID, WorldProjectile>;
    pub type Projectiles = PoolLinkedHashMap<TGameElementID, WorldProjectile>;

    #[derive(Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct ProjectileRenderInfo {
        pub ty: WeaponType,
        pub pos: vec2,
        pub vel: vec2,
    }
}
