pub mod projectile {
    use game_interface::events::{GameGrenadeEventEffect, GameGrenadeEventSound};
    use game_interface::types::game::GameEntityId;
    use game_interface::types::weapons::WeaponType;
    use hashlink::LinkedHashMap;
    use hiarc::Hiarc;
    use math::math::vector::vec2;
    use math::math::{length, lerp, normalize};
    use pool::datatypes::{PoolLinkedHashMap, PoolVec};
    use pool::pool::Pool;
    use pool::{recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};

    use shared_base::reusable::{CloneWithCopyableElements, ReusableCore};

    use crate::collision::collision::Collision;
    use crate::entities::character::character::{
        Character, CharacterDamageResult, DamageBy, DamageTypes,
    };
    use crate::entities::entity::entity::{
        calc_pos_and_vel, Entity, EntityInterface, EntityTickResult,
    };
    use crate::events::events::ProjectileEvent;
    use crate::simulation_pipe::simulation_pipe::{
        SimulationEntityEvents, SimulationEventWorldEntity, SimulationPipeProjectile,
    };
    use crate::state::state::TICKS_PER_SECOND;
    use crate::world::world::GameWorld;

    #[derive(Debug, Hiarc, Default, Serialize, Deserialize)]
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

    #[derive(Debug, Hiarc, Default, Copy, Clone, Serialize, Deserialize)]
    pub struct ProjectileCore {
        pub pos: vec2,
        pub vel: vec2,
        pub direction: vec2,
        pub life_span: i32,
        pub damage: u32,
        pub force: f32,
        pub is_explosive: bool,
        pub ty: WeaponType,
    }

    #[derive(Debug, Hiarc, Clone)]
    pub struct ProjectilePool {
        pub(crate) projectile_pool: Pool<PoolProjectiles>,
        pub(crate) projectile_reusable_cores_pool: Pool<ProjectileReusableCore>,
        pub(crate) projectile_helper: Pool<Vec<(GameEntityId, vec2)>>,
    }

    #[derive(Debug, Hiarc)]
    pub struct Projectile {
        pub(crate) base: Entity,
        pub(crate) core: ProjectileCore,
        pub(crate) reusable_core: PoolProjectileReusableCore,

        simulation_events: SimulationEntityEvents,

        helper_ids: PoolVec<(GameEntityId, vec2)>,
    }

    impl Projectile {
        pub fn new(
            game_el_id: &GameEntityId,
            pos: &vec2,
            direction: &vec2,
            life_span: i32,
            damage: u32,
            force: f32,
            explosive: bool,
            ty: WeaponType,
            pool: &ProjectilePool,
            simulation_events: &SimulationEntityEvents,
        ) -> Self {
            let core = ProjectileCore {
                pos: *pos,
                vel: *direction,
                direction: *direction,
                life_span,
                damage,
                force,
                is_explosive: explosive,
                ty,
            };
            Self {
                base: Entity::new(game_el_id),
                core,
                reusable_core: pool.projectile_reusable_cores_pool.new(),

                simulation_events: simulation_events.clone(),

                helper_ids: pool.projectile_helper.new(),
            }
        }

        pub fn from(other: &Self, pool: &mut ProjectilePool) -> Self {
            let mut reusable_core = pool.projectile_reusable_cores_pool.new();
            reusable_core.copy_clone_from(&other.reusable_core);
            Self {
                base: Entity::new(&other.base.game_element_id),
                core: other.core,
                reusable_core,

                simulation_events: other.simulation_events.clone(),
                helper_ids: pool.projectile_helper.new(),
            }
        }

        fn advance_pos_and_dir(
            collision: &Collision,
            core: &mut ProjectileCore,
            pos: &mut vec2,
            time: f32,
        ) {
            let tuning = collision.get_tune_at(&core.pos);

            let curvature;
            let speed;

            match core.ty {
                WeaponType::Grenade => {
                    curvature = tuning.grenade_curvature;
                    speed = tuning.grenade_speed;
                }
                WeaponType::Shotgun => {
                    curvature = tuning.shotgun_curvature;
                    speed = tuning.shotgun_speed;
                }
                WeaponType::Gun => {
                    curvature = tuning.gun_curvature;
                    speed = tuning.gun_speed;
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

        fn create_explosion(&mut self, no_dmg: bool, pipe: &mut SimulationPipeProjectile) {
            // deal damage
            let radius = 135;
            let inner_radius = 48.0;
            let characters = pipe.characters_helper.get_characters();
            let intersections =
                GameWorld::intersect_characters(pipe.field, characters, &self.core.pos, radius);

            self.helper_ids.clear();
            self.helper_ids.extend(intersections.map(|character| {
                let diff = *character.pos.pos() - self.core.pos;
                (character.base.game_element_id, diff)
            }));

            for (id, diff) in self.helper_ids.drain(..) {
                let mut force_dir = vec2::new(0.0, 1.0);
                let mut l = length(&diff);
                if l > 0.0 {
                    force_dir = normalize(&diff);
                }
                l = 1.0 - ((l - inner_radius) / (radius as f32 - inner_radius)).clamp(0.0, 1.0);
                let strength = pipe
                    .collision
                    .get_tune_at(&self.core.pos)
                    .explosion_strength;

                let dmg = strength * l;
                if dmg <= 0.0 {
                    continue;
                }

                if Character::take_damage(
                    pipe.characters_helper.characters,
                    &id,
                    &(force_dir * dmg * 2.0),
                    &self.core.pos,
                    if no_dmg { 0 } else { dmg as u32 },
                    DamageTypes::Character(&pipe.characters_helper.owner_character),
                    DamageBy::Weapon(self.core.ty),
                ) == CharacterDamageResult::Death
                {
                    pipe.characters_helper.characters.remove(&id);
                }
            }
        }
    }

    impl<'a> EntityInterface<ProjectileCore, ProjectileReusableCore, SimulationPipeProjectile<'a>>
        for Projectile
    {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipeProjectile) -> EntityTickResult {
            todo!()
        }

        fn tick(&mut self, pipe: &mut SimulationPipeProjectile) -> EntityTickResult {
            let ticks_per_second = TICKS_PER_SECOND;
            let prev_pos = self.core.pos; // Self::get_pos(core, pt);
            let mut cur_pos = self.core.pos;
            Self::advance_pos_and_dir(
                pipe.collision,
                &mut self.core,
                &mut cur_pos,
                1.0 / (ticks_per_second as f32),
            );
            let mut dummy_pos = Default::default();
            let mut dummy_tele_nr = Default::default();
            let collide = pipe.collision.intersect_line_tele_hook(
                &prev_pos,
                &cur_pos.clone(),
                &mut cur_pos,
                &mut dummy_pos,
                &mut dummy_tele_nr,
            );

            self.core.life_span -= 1;

            let intersection = GameWorld::intersect_character_on_line(
                pipe.field,
                pipe.characters_helper.get_characters_except_owner(),
                &prev_pos,
                &cur_pos,
                6.0,
            );

            let res = if intersection.is_some()
                || collide > 0
                || self.core.life_span < 0
                || Entity::outside_of_playfield(&cur_pos, pipe.collision)
            {
                if self.core.life_span >= 0 || self.core.ty == WeaponType::Grenade {
                    //ent.entity_events.push(EntitiyEvent::Sound {}); // TODO: GameServer()->CreateSound(cur_pos, m_SoundImpact);
                }

                if self.core.is_explosive {
                    self.simulation_events
                        .push(SimulationEventWorldEntity::Projectile {
                            id: self.base.game_element_id,
                            ev: ProjectileEvent::GrenadeSound {
                                pos: self.core.pos / 32.0,
                                ev: GameGrenadeEventSound::Explosion,
                            },
                        });
                    self.simulation_events
                        .push(SimulationEventWorldEntity::Projectile {
                            id: self.base.game_element_id,
                            ev: ProjectileEvent::GrenadeEffect {
                                pos: self.core.pos / 32.0,
                                ev: GameGrenadeEventEffect::Explosion,
                            },
                        });
                    self.create_explosion(false, pipe);
                } else if let Some((_, _, intersect_char)) = intersection {
                    let intersect_char_id = intersect_char.base.game_element_id;
                    if Character::take_damage(
                        pipe.characters_helper.characters,
                        &intersect_char_id,
                        &(self.core.vel * 0.001_f32.max(self.core.force)),
                        &(self.core.vel * -1.0),
                        self.core.damage,
                        DamageTypes::Character(&pipe.characters_helper.owner_character),
                        DamageBy::Weapon(self.core.ty),
                    ) == CharacterDamageResult::Death
                    {
                        pipe.characters_helper.characters.remove(&intersect_char_id);
                    }
                }
                EntityTickResult::RemoveEntity
            } else {
                EntityTickResult::None
            };
            self.core.pos = cur_pos;
            res
        }

        fn tick_deferred(&mut self, _pipe: &mut SimulationPipeProjectile) -> EntityTickResult {
            // TODO: todo!()
            EntityTickResult::None
        }

        fn drop_silent(&mut self) {
            self.base.drop_silent = true;
        }
    }

    impl Drop for Projectile {
        fn drop(&mut self) {
            if !self.base.drop_silent {
                self.simulation_events
                    .push(SimulationEventWorldEntity::Projectile {
                        id: self.base.game_element_id,
                        ev: ProjectileEvent::Despawn {
                            pos: self.core.pos,
                            respawns_in_ticks: 0.into(),
                        },
                    });
            }
        }
    }

    pub fn lerped_pos(proj1: &Projectile, proj2: &Projectile, ratio: f64) -> vec2 {
        lerp(&proj1.core.pos, &proj2.core.pos, ratio as f32)
    }
    pub fn estimated_fly_direction(proj1: &Projectile, proj2: &Projectile, ratio: f64) -> vec2 {
        lerp(&proj1.core.vel, &proj2.core.vel, ratio as f32)
    }

    #[derive(Debug, Hiarc)]
    pub struct WorldProjectile {
        pub character_id: GameEntityId,
        pub projectile: Projectile,
    }

    pub type PoolProjectiles = LinkedHashMap<GameEntityId, WorldProjectile>;
    pub type Projectiles = PoolLinkedHashMap<GameEntityId, WorldProjectile>;
}
