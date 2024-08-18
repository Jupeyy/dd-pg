pub mod laser {
    use game_interface::events::GameLaserEventSound;
    use game_interface::types::game::{
        GameEntityId, GameTickCooldownAndLastActionCounter, GameTickType, NonZeroGameTickType,
    };
    use game_interface::types::weapons::WeaponType;
    use hashlink::LinkedHashMap;
    use hiarc::Hiarc;
    use math::math::vector::vec2;
    use math::math::{distance, normalize};
    use pool::datatypes::PoolLinkedHashMap;
    use pool::pool::Pool;
    use pool::{recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};

    use game_interface::types::laser::LaserType;
    use shared_base::reusable::{CloneWithCopyableElements, ReusableCore};

    use crate::entities::character::character::{
        Character, CharacterDamageResult, DamageBy, DamageTypes,
    };
    use crate::entities::entity::entity::{Entity, EntityInterface, EntityTickResult};
    use crate::events::events::LaserEvent;
    use crate::simulation_pipe::simulation_pipe::{
        SimulationEntityEvents, SimulationEventWorldEntity, SimulationPipeLaser,
    };
    use crate::state::state::TICKS_PER_SECOND;
    use crate::world::world::GameWorld;

    #[derive(Debug, Hiarc, Default, Serialize, Deserialize)]
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

    #[derive(Debug, Hiarc, Default, Copy, Clone, Serialize, Deserialize)]
    pub struct LaserCore {
        pub pos: vec2,
        pub from: vec2,
        pub dir: vec2,
        pub ty: LaserType,

        pub energy: f32,
        pub bounces: usize,
        pub next_eval_in: GameTickCooldownAndLastActionCounter,
        // TODO: int m_Owner;
        // TODO: int m_TeamMask;

        // can this entity hit players and own player
        pub can_hit_others: bool,
        pub can_hit_own: bool,
    }

    #[derive(Debug, Hiarc, Clone)]
    pub struct LaserPool {
        pub(crate) laser_pool: Pool<PoolLasers>,
        pub(crate) laser_reusable_cores_pool: Pool<LaserReusableCore>,
    }

    #[derive(Debug, Hiarc)]
    pub struct Laser {
        pub(crate) base: Entity,
        pub(crate) core: LaserCore,
        pub(crate) reusable_core: PoolLaserReusableCore,

        simulation_events: SimulationEntityEvents,
    }

    impl Laser {
        pub fn new(
            game_el_id: &GameEntityId,
            pos: &vec2,
            dir: &vec2,
            start_energy: f32,

            can_hit_others: bool,
            can_hit_own: bool,

            pool: &LaserPool,
            simulation_events: &SimulationEntityEvents,
        ) -> Self {
            let core = LaserCore {
                pos: *pos,
                from: *pos,
                ty: LaserType::Rifle,
                bounces: 0,
                dir: *dir,
                energy: start_energy,
                next_eval_in: Default::default(),

                can_hit_others,
                can_hit_own,
            };

            Self {
                base: Entity::new(game_el_id),
                core,
                reusable_core: pool.laser_reusable_cores_pool.new(),
                simulation_events: simulation_events.clone(),
            }
        }

        pub fn from(other: &Self, pool: &mut LaserPool) -> Self {
            let mut reusable_core = pool.laser_reusable_cores_pool.new();
            reusable_core.copy_clone_from(&other.reusable_core);
            Self {
                base: Entity::new(&other.base.game_element_id),
                core: other.core,
                reusable_core,

                simulation_events: other.simulation_events.clone(),
            }
        }

        pub fn pos(&self) -> vec2 {
            self.core.pos
        }

        pub fn pos_from(&self) -> vec2 {
            self.core.from
        }

        pub fn eval_tick_ratio(&self) -> Option<(GameTickType, NonZeroGameTickType)> {
            self.core.next_eval_in.action_ticks_and_cooldown_len()
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

        fn hit_character(
            &mut self,
            pipe: &mut SimulationPipeLaser,
            from: &vec2,
            to: &vec2,
        ) -> bool {
            let dont_hit_self = self.core.bounces == 0;

            let mut char_intersection = None;
            if self.core.can_hit_others {
                let intersection = if self.core.can_hit_own && !dont_hit_self {
                    GameWorld::intersect_character_on_line(
                        pipe.field,
                        pipe.characters_helper.get_characters(),
                        &self.core.pos,
                        to,
                        0.0,
                    )
                } else {
                    GameWorld::intersect_character_on_line(
                        pipe.field,
                        pipe.characters_helper.get_characters_except_owner(),
                        &self.core.pos,
                        to,
                        0.0,
                    )
                };
                char_intersection = intersection;
            } else if self.core.can_hit_own && !dont_hit_self {
                // check if owner was hit
                let intersection = GameWorld::intersect_character_on_line(
                    pipe.field,
                    pipe.characters_helper.get_owner_character_view(),
                    &self.core.pos,
                    to,
                    0.0,
                );
                char_intersection = intersection;
            }

            let Some((_, pos, char)) = char_intersection else {
                return false;
            };
            self.core.from = *from;
            self.core.pos = pos;
            self.core.energy = -1.0;

            if let LaserType::Shotgun = self.core.ty {
                /* TODO: ddrace
                vec2 Temp;

                float Strength;
                if(!m_TuneZone)
                    Strength = GameServer()->Tuning()->m_ShotgunStrength;
                else
                    Strength = GameServer()->TuningList()[m_TuneZone].m_ShotgunStrength;

                vec2 &HitPos = pHit->Core()->m_Pos;
                if(!g_Config.m_SvOldLaser)
                {
                    if(m_PrevPos != HitPos)
                    {
                        Temp = pHit->Core()->m_Vel + normalize(m_PrevPos - HitPos) * Strength;
                        pHit->Core()->m_Vel = ClampVel(pHit->m_MoveRestrictions, Temp);
                    }
                    else
                    {
                        pHit->Core()->m_Vel = StackedLaserShotgunBugSpeed;
                    }
                }
                else if(g_Config.m_SvOldLaser && pOwnerChar)
                {
                    if(pOwnerChar->Core()->m_Pos != HitPos)
                    {
                        Temp = pHit->Core()->m_Vel + normalize(pOwnerChar->Core()->m_Pos - HitPos) * Strength;
                        pHit->Core()->m_Vel = ClampVel(pHit->m_MoveRestrictions, Temp);
                    }
                    else
                    {
                        pHit->Core()->m_Vel = StackedLaserShotgunBugSpeed;
                    }
                }
                else
                {
                    pHit->Core()->m_Vel = ClampVel(pHit->m_MoveRestrictions, pHit->Core()->m_Vel);
                }*/
            } else if let LaserType::Rifle = self.core.ty {
                let dmg_amount = pipe.collision.get_tune_at(&self.core.pos).laser_damage;
                let hitted_char_id = char.base.game_element_id;
                if Character::take_damage(
                    pipe.characters_helper.characters,
                    &hitted_char_id,
                    &Default::default(),
                    &Default::default(),
                    dmg_amount as u32,
                    DamageTypes::Character(&pipe.characters_helper.owner_character),
                    DamageBy::Weapon(WeaponType::Laser),
                ) == CharacterDamageResult::Death
                {
                    pipe.characters_helper.characters.remove(&hitted_char_id);
                }
            }
            true
        }

        fn do_bounce(&mut self, pipe: &mut SimulationPipeLaser) -> bool {
            let tuning = pipe.collision.get_tune_at(&self.core.pos);
            let delay = tuning.laser_bounce_delay;
            self.core.next_eval_in =
                ((TICKS_PER_SECOND as f32 * delay / 1000.0).ceil() as GameTickType).into();

            if self.core.energy < 0.0 {
                return false;
            }
            //self.core.m_PrevPos = self.core.pos;
            let mut col_tile = vec2::default();

            let mut z = 0;

            if false
            // TODO: (m_WasTele)
            {
                /*self.core.m_PrevPos = self.core.m_TelePos;
                self.core.pos = self.core.m_TelePos;
                self.core.m_TelePos = vec2::new(0.0, 0.0);*/
            }

            let mut to = self.core.pos + self.core.dir * self.core.energy;

            let res = pipe.collision.intersect_line_tele_hook(
                &self.core.pos,
                &to.clone(),
                &mut col_tile,
                &mut to,
                &mut z,
            );

            if res > 0 {
                let cur_pos = self.core.pos;
                if !self.hit_character(pipe, &cur_pos, &to) {
                    let core = &mut self.core;
                    // intersected
                    core.from = core.pos;
                    core.pos = to;

                    let mut tmp_pos = core.pos;
                    let mut tmp_dir = core.dir * 4.0;

                    // TODO: let mut f = 0;
                    // TODO: this looks like a hack, maybe remove it completely
                    if res == -1 {
                        // TODO: f = GameServer()->Collision()->GetTile(round_to_int(Coltile.x), round_to_int(Coltile.y));
                        // TODO: GameServer()->Collision()->SetCollisionAt(round_to_int(Coltile.x), round_to_int(Coltile.y), TILE_SOLID);
                    }
                    pipe.collision
                        .move_point(&mut tmp_pos, &mut tmp_dir, 1.0, &mut 0);
                    if res == -1 {
                        // TODO:   GameServer()->Collision()->SetCollisionAt(round_to_int(Coltile.x), round_to_int(Coltile.y), f);
                    }
                    core.pos = tmp_pos;
                    core.dir = normalize(&mut tmp_dir);

                    let d = distance(&core.from, &core.pos);
                    // Prevent infinite bounces
                    if core.bounces > 0 && d == 0.0 {
                        core.energy = -1.0;
                    } else {
                        let tuning = pipe.collision.get_tune_at(&core.pos);
                        core.energy -= d + tuning.laser_bounce_cost;
                    }

                    // TODO: CGameControllerDDRace *pControllerDDRace = (CGameControllerDDRace *)GameServer()->m_pController;
                    if false
                    // TODO: (Res == TILE_TELEINWEAPON && !pControllerDDRace->m_TeleOuts[z - 1].empty())
                    {
                        /* TODO: int TeleOut = GameServer()->m_World.m_Core.RandomOr0(pControllerDDRace->m_TeleOuts[z - 1].size());
                        m_TelePos = pControllerDDRace->m_TeleOuts[z - 1][TeleOut];
                        m_WasTele = true;*/
                    } else {
                        core.bounces += 1;
                        //core.m_WasTele = false;
                    }

                    let tuning = pipe.collision.get_tune_at(&core.pos);
                    let bounce_num = tuning.laser_bounce_num as usize;

                    if core.bounces > bounce_num {
                        core.energy = -1.0;
                    }

                    self.simulation_events
                        .push(SimulationEventWorldEntity::Laser {
                            id: self.base.game_element_id,
                            ev: LaserEvent::Sound {
                                pos: core.pos,
                                ev: GameLaserEventSound::Bounce,
                            },
                        });
                }
            } else {
                let cur_pos = self.core.pos;
                if !self.hit_character(pipe, &cur_pos, &to) {
                    self.core.from = self.core.pos;
                    self.core.pos = to;
                    self.core.energy = -1.0;
                }
            }

            /*CCharacter *pOwnerChar = GameServer()->GetPlayerChar(m_Owner);
            if(m_Owner >= 0 && m_Energy <= 0 && !m_TeleportCancelled && pOwnerChar &&
                pOwnerChar->IsAlive() && pOwnerChar->HasTelegunLaser() && m_Type == WEAPON_LASER)
            {
                vec2 PossiblePos;
                bool Found = false;

                // Check if the laser hits a player.
                bool pDontHitSelf = g_Config.m_SvOldLaser || (m_Bounces == 0 && !m_WasTele);
                vec2 At;
                CCharacter *pHit;
                if(pOwnerChar ? (!pOwnerChar->LaserHitDisabled() && m_Type == WEAPON_LASER) : g_Config.m_SvHit)
                    pHit = GameServer()->m_World.IntersectCharacter(m_Pos, To, 0.f, At, pDontHitSelf ? pOwnerChar : 0, m_Owner);
                else
                    pHit = GameServer()->m_World.IntersectCharacter(m_Pos, To, 0.f, At, pDontHitSelf ? pOwnerChar : 0, m_Owner, pOwnerChar);

                if(pHit)
                    Found = GetNearestAirPosPlayer(pHit->m_Pos, &PossiblePos);
                else
                    Found = GetNearestAirPos(m_Pos, m_From, &PossiblePos);

                if(Found)
                {
                    pOwnerChar->m_TeleGunPos = PossiblePos;
                    pOwnerChar->m_TeleGunTeleport = true;
                    pOwnerChar->m_IsBlueTeleGunTeleport = m_IsBlueTeleport;
                }
            }
            else if(m_Owner >= 0)
            {
                int MapIndex = GameServer()->Collision()->GetPureMapIndex(Coltile);
                int TileFIndex = GameServer()->Collision()->GetFTileIndex(MapIndex);
                bool IsSwitchTeleGun = GameServer()->Collision()->GetSwitchType(MapIndex) == TILE_ALLOW_TELE_GUN;
                bool IsBlueSwitchTeleGun = GameServer()->Collision()->GetSwitchType(MapIndex) == TILE_ALLOW_BLUE_TELE_GUN;
                int IsTeleInWeapon = GameServer()->Collision()->IsTeleportWeapon(MapIndex);

                if(!IsTeleInWeapon)
                {
                    if(IsSwitchTeleGun || IsBlueSwitchTeleGun)
                    {
                        // Delay specifies which weapon the tile should work for.
                        // Delay = 0 means all.
                        int delay = GameServer()->Collision()->GetSwitchDelay(MapIndex);

                        if((delay != 3 && delay != 0) && m_Type == WEAPON_LASER)
                        {
                            IsSwitchTeleGun = IsBlueSwitchTeleGun = false;
                        }
                    }

                    m_IsBlueTeleport = TileFIndex == TILE_ALLOW_BLUE_TELE_GUN || IsBlueSwitchTeleGun;

                    // Teleport is canceled if the last bounce tile is not a TILE_ALLOW_TELE_GUN.
                    // Teleport also works if laser didn't bounce.
                    m_TeleportCancelled =
                        m_Type == WEAPON_LASER && (TileFIndex != TILE_ALLOW_TELE_GUN && TileFIndex != TILE_ALLOW_BLUE_TELE_GUN && !IsSwitchTeleGun && !IsBlueSwitchTeleGun);
                }
            }*/

            true
        }

        pub fn lerped_pos(laser1: &Laser, _laser2: &Laser, _ratio: f64) -> vec2 {
            laser1.core.pos
        }
        pub fn lerped_from(laser1: &Laser, _laser2: &Laser, _ratio: f64) -> vec2 {
            laser1.core.from
        }
    }

    impl<'a> EntityInterface<LaserCore, LaserReusableCore, SimulationPipeLaser<'a>> for Laser {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipeLaser) -> EntityTickResult {
            todo!()
        }

        fn tick(&mut self, pipe: &mut SimulationPipeLaser) -> EntityTickResult {
            /* TODO: if((g_Config.m_SvDestroyLasersOnDeath || m_BelongsToPracticeTeam) && m_Owner >= 0)
            {
                CCharacter *pOwnerChar = GameServer()->GetPlayerChar(m_Owner);
                if(!(pOwnerChar && pOwnerChar->IsAlive()))
                {
                    Self::laser_die(pipe);
                }
            }*/

            let owner_char = pipe.characters_helper.get_owner_character();
            if self
                .core
                .next_eval_in
                .tick()
                .cooldown_fell_to_zero_or_none()
            {
                if self.do_bounce(pipe) {
                    EntityTickResult::None
                } else {
                    EntityTickResult::RemoveEntity
                }
            } else {
                EntityTickResult::None
            }
        }

        fn tick_deferred(&mut self, _pipe: &mut SimulationPipeLaser) -> EntityTickResult {
            EntityTickResult::None
        }

        fn drop_silent(&mut self) {
            self.base.drop_silent = true;
        }
    }

    impl Drop for Laser {
        fn drop(&mut self) {
            if !self.base.drop_silent {
                self.simulation_events
                    .push(SimulationEventWorldEntity::Laser {
                        id: self.base.game_element_id,
                        ev: LaserEvent::Despawn {
                            pos: self.core.pos,
                            respawns_in_ticks: 0.into(),
                        },
                    });
            }
        }
    }

    #[derive(Debug, Hiarc)]
    pub struct WorldLaser {
        pub character_id: GameEntityId,
        pub laser: Laser,
    }

    pub type PoolLasers = LinkedHashMap<GameEntityId, WorldLaser>;
    pub type Lasers = PoolLinkedHashMap<GameEntityId, WorldLaser>;
}
