pub mod laser {
    use base_log::log::SystemLogGroup;
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::vector::vec2;
    use math::math::{distance, normalize};
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
    use crate::entities::entity::entity::{Entity, EntityInterface};
    use crate::events::events::{EntityEvent, LaserEvent};
    use crate::simulation_pipe::simulation_pipe::SimulationPipeLaser;
    use crate::state::state::TICKS_PER_SECOND;
    use crate::world::world::GameWorld;

    #[derive(Debug, Default, Serialize, Deserialize, Encode, Decode)]
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
    pub type MtPoolLaserReusableCore = MtRecycle<LaserReusableCore>;

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
    pub enum LaserType {
        #[default]
        Rifle,
        Shotgun, // TODO: rename to puller
        Door,
        Freeze,
    }

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
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

        // can this entity hit players and own player
        pub can_hit_others: bool,
        pub can_hit_own: bool,

        // DDRace
        m_TelePos: vec2,
        m_WasTele: bool,
        m_PrevPos: vec2,
        m_Type: i32,
        m_TuneZone: i32,
        m_TeleportCancelled: bool,
        m_IsBlueTeleport: bool,
        m_BelongsToPracticeTeam: bool,
    }

    #[derive(Debug, Clone)]
    pub struct LaserPool {
        pub(crate) laser_pool: Pool<PoolLasers>,
        pub(crate) laser_reusable_cores_pool: Pool<LaserReusableCore>,
    }

    #[derive(Debug)]
    pub struct Laser {
        pub(crate) base: Entity,
        core: LaserCore,
        reusable_core: PoolLaserReusableCore,

        pub(crate) entity_events: Vec<LaserEvent>,
    }

    impl Laser {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            pos: &vec2,
            dir: &vec2,
            start_tick: GameTickType,
            start_energy: f32,

            can_hit_others: bool,
            can_hit_own: bool,

            pool: &LaserPool,
        ) -> Self {
            let cur_pos = *pos + *dir * 800.0; // TODO:

            let core = LaserCore {
                pos: cur_pos,
                from: *pos,
                start_tick,
                ty: LaserType::Rifle,
                bounces: 0,
                dir: *dir,
                energy: start_energy,
                eval_tick: start_tick,

                can_hit_others,
                can_hit_own,

                // ddrace
                m_TelePos: vec2::default(),
                m_WasTele: bool::default(),
                m_PrevPos: vec2::default(),
                m_Type: i32::default(),
                m_TuneZone: i32::default(),
                m_TeleportCancelled: bool::default(),
                m_IsBlueTeleport: bool::default(),
                m_BelongsToPracticeTeam: bool::default(),
            };

            Self {
                base: Entity::new(game_el_id, logger),
                core,
                reusable_core: pool.laser_reusable_cores_pool.new(),
                entity_events: Default::default(),
            }
        }

        pub fn from(other: &Self, logger: SystemLogGroup, pool: &mut LaserPool) -> Self {
            let mut reusable_core = pool.laser_reusable_cores_pool.new();
            reusable_core.copy_clone_from(&other.reusable_core);
            Self {
                base: Entity::new(&other.base.game_element_id, logger),
                core: other.core,
                reusable_core,

                entity_events: Default::default(),
            }
        }

        pub fn pos(&self) -> vec2 {
            self.core.pos
        }

        pub fn pos_from(&self) -> vec2 {
            self.core.from
        }

        pub fn start_tick(&self) -> GameTickType {
            self.core.start_tick
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
            let StackedLaserShotgunBugSpeed = vec2::new(-2147483648.0, -2147483648.0);

            let pOwnerChar = pipe.characters_helper.get_owner_character();
            let pDontHitSelf = true; // TODO: g_Config.m_SvOldLaser || (m_Bounces == 0 && !m_WasTele);

            let mut pHit = None;
            if self.core.can_hit_others {
                let intersection = if self.core.can_hit_own {
                    GameWorld::intersect_character(
                        pipe.characters_helper.get_characters(),
                        &self.core.pos,
                        &to,
                        0.0,
                    )
                } else {
                    GameWorld::intersect_character(
                        pipe.characters_helper.get_characters_except_owner(),
                        &self.core.pos,
                        &to,
                        0.0,
                    )
                };
                pHit = intersection;
            } else if self.core.can_hit_own {
                // check if owner was hit
                let intersection = GameWorld::intersect_character(
                    pipe.characters_helper.get_owner_character_it(),
                    &self.core.pos,
                    &to,
                    0.0,
                );
                pHit = intersection;
            }

            let Some((_, pos, char)) = pHit else {
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
                let hitted_char_id = char.base.game_element_id;
                Character::take_damage(
                    pipe.characters_helper.characters,
                    &hitted_char_id,
                    pipe.cur_tick,
                    &Default::default(),
                    &Default::default(),
                    0, // TODO:
                    DamageTypes::Character(&pipe.characters_helper.owner_character),
                    WeaponType::Laser,
                );
            }
            true
        }

        fn laser_die(&mut self) {
            self.entity_events.push(LaserEvent::Despawn {
                pos: self.core.pos,
                respawns_at_tick: None,
            });
        }

        fn do_bounce(&mut self, pipe: &mut SimulationPipeLaser) {
            self.core.eval_tick = pipe.cur_tick;

            if self.core.energy < 0.0 {
                self.laser_die();
                return;
            }
            self.core.m_PrevPos = self.core.pos;
            let mut col_tile = vec2::default();

            let mut z = 0;

            if false
            // TODO: (m_WasTele)
            {
                self.core.m_PrevPos = self.core.m_TelePos;
                self.core.pos = self.core.m_TelePos;
                self.core.m_TelePos = vec2::new(0.0, 0.0);
            }

            let mut To = self.core.pos + self.core.dir * self.core.energy;

            let res = pipe.collision.intersect_line_tele_hook(
                &self.core.pos,
                &To.clone(),
                &mut col_tile,
                &mut To,
                &mut z,
            );

            if res > 0 {
                let cur_pos = self.core.pos;
                if !self.hit_character(pipe, &cur_pos, &To) {
                    let core = &mut self.core;
                    // intersected
                    core.from = core.pos;
                    core.pos = To;

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
                    if d == 0.0
                    // TODO: && m_ZeroEnergyBounceInLastTick)
                    {
                        core.energy = -1.0;
                    } else if true
                    // TODO: (!m_TuneZone)
                    {
                        core.energy -= d + 400.0 // TODO: <- GameServer()->Tuning()->m_LaserBounceCost;
                    } else {
                        // TODO: core.energy -= distance(m_From, m_Pos) + GameServer()->TuningList()[m_TuneZone].m_LaserBounceCost;
                    }
                    // TODO: m_ZeroEnergyBounceInLastTick = Distance == 0.0f;

                    // TODO: CGameControllerDDRace *pControllerDDRace = (CGameControllerDDRace *)GameServer()->m_pController;
                    if false
                    // TODO: (Res == TILE_TELEINWEAPON && !pControllerDDRace->m_TeleOuts[z - 1].empty())
                    {
                        /* TODO: int TeleOut = GameServer()->m_World.m_Core.RandomOr0(pControllerDDRace->m_TeleOuts[z - 1].size());
                        m_TelePos = pControllerDDRace->m_TeleOuts[z - 1][TeleOut];
                        m_WasTele = true;*/
                    } else {
                        core.bounces += 1;
                        core.m_WasTele = false;
                    }

                    let bounce_num = 1; // TODO: <- GameServer()->Tuning()->m_LaserBounceNum;
                                        /* TODO: if m_TuneZone {
                                            BounceNum = GameServer()->TuningList()[m_TuneZone].m_LaserBounceNum;
                                        }*/

                    if core.bounces > bounce_num {
                        core.energy = -1.0;
                    }

                    // TODO: GameServer()->CreateSound(m_Pos, SOUND_LASER_BOUNCE, m_TeamMask);
                    self.entity_events
                        .push(LaserEvent::Entity(EntityEvent::Sound {
                            pos: core.pos,
                            name: "weapon/laser_bounce".to_string(),
                        }));
                }
            } else {
                let cur_pos = self.core.pos;
                if !self.hit_character(pipe, &cur_pos, &To) {
                    self.core.from = self.core.pos;
                    self.core.pos = To;
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
        }
    }

    impl<'a> EntityInterface<LaserCore, LaserReusableCore, SimulationPipeLaser<'a>> for Laser {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipeLaser) {
            todo!()
        }

        fn tick(&mut self, pipe: &mut SimulationPipeLaser) {
            /* TODO: if((g_Config.m_SvDestroyLasersOnDeath || m_BelongsToPracticeTeam) && m_Owner >= 0)
            {
                CCharacter *pOwnerChar = GameServer()->GetPlayerChar(m_Owner);
                if(!(pOwnerChar && pOwnerChar->IsAlive()))
                {
                    Self::laser_die(pipe);
                }
            }*/

            let owner_char = pipe.characters_helper.get_owner_character();
            let delay;
            if false
            // TODO: (m_TuneZone)
            {
                delay = 0.0; // TODO: GameServer()->TuningList()[m_TuneZone].m_LaserBounceDelay;
            } else {
                delay = 125.0; // TODO: owner_chat.get_core_at_index(pipe.cur_core_index).core; GameServer()->Tuning()->m_LaserBounceDelay;
            }

            if (pipe.cur_tick - self.core.eval_tick) as f32
                > (TICKS_PER_SECOND as f32 * delay / 1000.0)
            {
                self.do_bounce(pipe);
            }
        }

        fn tick_deferred(&mut self, _pipe: &mut SimulationPipeLaser) {}

        fn split(&self) -> (&Entity, &LaserCore, &Recycle<LaserReusableCore>) {
            (&self.base, &self.core, &self.reusable_core)
        }

        fn split_mut(&mut self) -> (&mut Entity, &mut LaserCore, &mut Recycle<LaserReusableCore>) {
            (&mut self.base, &mut self.core, &mut self.reusable_core)
        }
    }

    #[derive(Debug)]
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
