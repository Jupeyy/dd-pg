pub mod character {
    use base_log::log::SystemLogGroup;
    use shared_base::{
        game_types::TGameElementID,
        network::messages::{InputVarClickable, WeaponType, NUM_WEAPONS},
        reuseable::{CloneWithCopyableElements, ReusableCore},
        types::GameTickType,
    };

    use crate::{
        entities::{
            character_core::character_core::{Core, CorePipe, CoreReusable},
            entity::entity::{Entity, EntityInterface},
        },
        events::events::{CharacterEvent, EntityEvent},
        player::player::{PlayerInfo, PlayerInput},
        simulation_pipe::simulation_pipe::SimulationPipeCharacter,
        state::state::TICKS_PER_SECOND,
        types::types::{GameOptions, GameTeam, GameType},
        weapons::definitions::weapon_def::Weapon,
    };

    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::{angle, length, lerp, normalize, vector::vec2};
    use pool::{
        datatypes::PoolLinkedHashMap, mt_recycle::Recycle as MtRecycle, pool::Pool,
        recycle::Recycle, traits::Recyclable,
    };
    use serde::{Deserialize, Serialize};

    pub const PHYSICAL_SIZE: f32 = 28.0;
    pub const TICKS_UNTIL_RECOIL_ENDED: GameTickType = 7;

    pub enum CharacterTeam {
        Red,
        Blue,
    }

    pub enum DamageTypes<'a> {
        Character(&'a TGameElementID),
        Team(CharacterTeam),
    }

    #[derive(Debug, Default, Serialize, Deserialize, Copy, Clone, Encode, Decode)]
    pub struct CharacterCore {
        pub core: Core,
        // vanilla
        pub active_weapon: WeaponType,
        pub prev_weapon: WeaponType,
        pub queued_weapon: Option<WeaponType>,
        pub weapon_diff: i32,
        pub health: u32,
        pub armor: u32,
        pub recoil_start_tick: GameTickType,
        pub recoil_tick_amount: GameTickType,
        pub recoil_click: InputVarClickable<bool>,

        pub score: i64,
        pub team: Option<GameTeam>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
    pub struct CharacterReusableCore {
        pub core: CoreReusable,
        #[bincode(with_serde)]
        pub weapons: LinkedHashMap<WeaponType, Weapon>,
    }

    impl CloneWithCopyableElements for CharacterReusableCore {
        fn copy_clone_from(&mut self, other: &Self) {
            self.core.copy_clone_from(&other.core);
            self.weapons.copy_clone_from(&other.weapons);
        }
    }

    impl Recyclable for CharacterReusableCore {
        fn new() -> Self {
            Self {
                core: CoreReusable::new(),
                weapons: Default::default(),
            }
        }
        fn reset(&mut self) {
            self.core.reset();
            self.weapons.reset();
        }
    }

    impl ReusableCore for CharacterReusableCore {}

    pub type PoolCharacterReusableCore = Recycle<CharacterReusableCore>;
    pub type MtPoolCharacterReusableCore = MtRecycle<CharacterReusableCore>;

    #[derive(Debug, Clone)]
    pub struct CharacterPool {
        pub(crate) character_pool: Pool<PoolCharacters>,
        pub(crate) character_reusable_cores_pool: Pool<CharacterReusableCore>,
    }

    #[derive(Debug)]
    pub struct Character {
        pub(crate) base: Entity,
        pub(crate) core: CharacterCore,
        pub(crate) reusable_core: PoolCharacterReusableCore,

        pub(crate) entity_events: Vec<CharacterEvent>,

        pub(crate) player_info: PlayerInfo,
        pub(crate) input: PlayerInput,
    }

    impl Character {
        pub fn new(
            game_el_id: &TGameElementID,
            logger: SystemLogGroup,
            character_pool: &CharacterPool,
            player_info: PlayerInfo,
            game_options: &GameOptions,
        ) -> Self {
            let mut core = CharacterCore::default();
            core.team = match game_options.ty {
                GameType::Solo => None,
                GameType::Team => Some(GameTeam::Blue),
            };

            Self {
                base: Entity::new(game_el_id, logger),
                core,
                reusable_core: character_pool.character_reusable_cores_pool.new(),

                entity_events: Default::default(),

                player_info,
                input: Default::default(),
            }
        }

        pub(crate) fn die(&mut self, cur_tick: GameTickType, killer_id: Option<TGameElementID>) {
            self.entity_events.push(CharacterEvent::Despawn {
                pos: self.core.core.pos,
                respawns_at_tick: Some(cur_tick + TICKS_PER_SECOND / 2),
                player_info: self.player_info.clone(),
                killer_id,
            });

            //int ModeSpecial = GameServer()->m_pController->OnCharacterDeath(this, (Killer < 0) ? 0 : GameServer()->m_apPlayers[Killer], Weapon);

            /*char aBuf[256];
            if(Killer < 0)
            {
                /*str_format(aBuf, sizeof(aBuf), "kill killer='%d:%d:' victim='%d:%d:%s' weapon=%d special=%d",
                    Killer, - 1 - Killer,
                    m_pPlayer->GetCID(), m_pPlayer->GetTeam(), Server()->ClientName(m_pPlayer->GetCID()), Weapon, ModeSpecial
                );*/
            }
            else
            {
                /*str_format(aBuf, sizeof(aBuf), "kill killer='%d:%d:%s' victim='%d:%d:%s' weapon=%d special=%d",
                    Killer, GameServer()->m_apPlayers[Killer]->GetTeam(), Server()->ClientName(Killer),
                    m_pPlayer->GetCID(), m_pPlayer->GetTeam(), Server()->ClientName(m_pPlayer->GetCID()), Weapon, ModeSpecial
                );*/
            }*/
            //GameServer()->Console()->Print(IConsole::OUTPUT_LEVEL_DEBUG, "game", aBuf);
            /*
                   // send the kill message
                   CNetMsg_Sv_KillMsg Msg;
                   Msg.m_Victim = m_pPlayer->GetCID();
                   Msg.m_ModeSpecial = ModeSpecial;
                   for(int i = 0 ; i < MAX_CLIENTS; i++)
                   {
                       if(!Server()->ClientIngame(i))
                           continue;

                       if(Killer < 0 && Server()->GetClientVersion(i) < MIN_KILLMESSAGE_CLIENTVERSION)
                       {
                           Msg.m_Killer = 0;
                           Msg.m_Weapon = WEAPON_WORLD;
                       }
                       else
                       {
                           Msg.m_Killer = Killer;
                           Msg.m_Weapon = Weapon;
                       }
                       Server()->SendPackMsg(&Msg, MSGFLAG_VITAL, i);
                   }

                   // a nice sound
                   GameServer()->CreateSound(m_Pos, SOUND_PLAYER_DIE);
            */
        }

        fn set_weapon(&mut self, new_weapon: WeaponType) {
            if self.core.active_weapon == new_weapon {
                return;
            }

            self.core.prev_weapon = self.core.active_weapon;
            self.core.queued_weapon = None;
            self.core.active_weapon = new_weapon;
            self.entity_events
                .push(CharacterEvent::Entity(EntityEvent::Sound {
                    pos: self.core.core.pos,
                    name: "weapon/switch".to_string(),
                }));
            // TODO: GameServer()->CreateSound(m_Pos, SOUND_WEAPON_SWITCH);

            if self.core.active_weapon as usize >= NUM_WEAPONS {
                self.core.active_weapon = Default::default(); // TODO: what is the idea behind this?
            }
            if let Some(_) = self.reusable_core.weapons.get_mut(&self.core.active_weapon) {
                // TODO: weapon.next_ammo_regeneration_tick
                //core.weapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
            }
        }

        fn do_weapon_switch(&mut self, cur_tick: GameTickType) {
            // make sure we can switch
            if (cur_tick - self.core.recoil_start_tick <= self.core.recoil_tick_amount)
                || self.core.queued_weapon.is_none()
            // TODO: ninja || reusable_core.weapons.contains_key(k) m_aWeapons[WEAPON_NINJA].m_Got
            {
                return;
            }

            // switch weapon
            self.set_weapon(self.core.queued_weapon.unwrap());
        }

        pub fn is_friendly_fire(
            _characters: &mut Characters,
            _self_char_id: &TGameElementID,
            _other_char_id: &TGameElementID,
        ) -> bool {
            false // TODO
        }

        pub fn is_friendly_fire_team(
            _characters: &mut Characters,
            _self_char_id: &TGameElementID,
            _other_team: CharacterTeam,
        ) -> bool {
            false // TODO
        }

        pub fn take_damage(
            characters: &mut Characters,
            self_char_id: &TGameElementID,
            cur_tick: GameTickType,
            force: &vec2,
            _source: &vec2,
            mut dmg_amount: u32,
            from: DamageTypes,
            _weapon: WeaponType,
        ) -> bool {
            let self_char = characters.get_mut(self_char_id).unwrap();
            let core = &mut self_char.core;
            core.core.vel += *force;

            let killer_id = match from {
                DamageTypes::Character(from_id) => {
                    if Self::is_friendly_fire(characters, self_char_id, from_id) {
                        return false;
                    }

                    // m_pPlayer only inflicts half damage on self
                    if from_id == self_char_id {
                        dmg_amount = 1.max(dmg_amount / 2);
                    }
                    Some(*from_id)
                }
                DamageTypes::Team(team) => {
                    if Self::is_friendly_fire_team(characters, self_char_id, team) {
                        return false;
                    }
                    None
                }
            };

            let self_char = characters.get_mut(self_char_id).unwrap();
            let (_, core, _) = &mut self_char.split_mut();
            let _old_health = core.health;
            let _old_armor = core.armor;
            if dmg_amount > 0 {
                if core.armor > 0 {
                    if dmg_amount > 1 {
                        core.health -= 1;
                        dmg_amount -= 1;
                    }

                    if dmg_amount > core.armor {
                        dmg_amount -= core.armor;
                        core.armor = 0;
                    } else {
                        core.armor -= dmg_amount.min(core.armor);
                        dmg_amount = 0;
                    }
                }

                core.health -= dmg_amount.min(core.health);
            }

            /* TODO:
            // create healthmod indicator
            GameServer()->CreateDamage(m_Pos, m_pPlayer->GetCID(), Source, OldHealth-m_Health, OldArmor-m_Armor, From == m_pPlayer->GetCID());

            // do damage Hit sound
            if(From >= 0 && From != m_pPlayer->GetCID() && GameServer()->m_apPlayers[From])
            {
                int64 Mask = CmaskOne(From);
                for(int i = 0; i < MAX_CLIENTS; i++)
                {
                    if(GameServer()->m_apPlayers[i] && (GameServer()->m_apPlayers[i]->GetTeam() == TEAM_SPECTATORS ||  GameServer()->m_apPlayers[i]->m_DeadSpecMode) &&
                        GameServer()->m_apPlayers[i]->GetSpectatorID() == From)
                        Mask |= CmaskOne(i);
                }
                GameServer()->CreateSound(GameServer()->m_apPlayers[From]->m_ViewPos, SOUND_HIT, Mask);
            }*/

            // check for death
            if core.health == 0 {
                self_char.die(cur_tick, killer_id);
                //self_char                    .entity_events                    .push(CharacterEvent::Killed { by_player });
                // TODO: Weapon -> Die(From, Weapon);

                // set attacker's face to happy (taunt!)
                /* TODO: if(From >= 0 && From != m_pPlayer->GetCID() && GameServer()->m_apPlayers[From])
                {
                    CCharacter *pChr = GameServer()->m_apPlayers[From]->GetCharacter();
                    if(pChr)
                    {
                        pChr->SetEmote(EMOTE_HAPPY, Server()->Tick() + Server()->TickSpeed());
                    }
                }*/

                return false;
            }

            /* TODO:
            if(Dmg > 2)
                GameServer()->CreateSound(m_Pos, SOUND_PLAYER_PAIN_LONG);
            else
                GameServer()->CreateSound(m_Pos, SOUND_PLAYER_PAIN_SHORT);

            SetEmote(EMOTE_PAIN, Server()->Tick() + 500 * Server()->TickSpeed() / 1000);

            return true;*/
            true
        }

        fn fire_weapon(&mut self, pipe: &mut SimulationPipeCharacter) {
            let cur_tick = pipe.cur_tick;
            let core = &mut self.core;

            if cur_tick - core.recoil_start_tick <= core.recoil_tick_amount {
                return;
            }

            self.do_weapon_switch(cur_tick);
            let core = &mut self.core;
            let input = &self.input.inp;
            let cursor_pos = input.cursor.to_vec2();
            let direction = normalize(&vec2::new(cursor_pos.x as f32, cursor_pos.y as f32));

            let full_auto = if core.active_weapon == WeaponType::Grenade
                || core.active_weapon == WeaponType::Shotgun
                || core.active_weapon == WeaponType::Laser
            {
                true
            } else {
                false
            };

            // check if we gonna fire
            let will_fire = input.fire.was_clicked(&core.recoil_click)
                || (full_auto && input.fire.is_currently_clicked());

            if !will_fire {
                return;
            }

            // check for ammo
            let cur_weapon = self.reusable_core.weapons.get(&core.active_weapon);
            if cur_weapon.is_some() && cur_weapon.unwrap().cur_ammo == 0 {
                // 125ms is a magical limit of how fast a human can click
                //m_ReloadTimer = 125 * Server()->TickSpeed() / 1000;
                /* TODO: if(m_LastNoAmmoSound+Server()->TickSpeed() <= Server()->Tick())
                {
                    GameServer()->CreateSound(m_Pos, SOUND_WEAPON_NOAMMO);
                    m_LastNoAmmoSound = Server()->Tick();
                }*/
                return;
            }

            let proj_start_pos = core.core.pos + direction * PHYSICAL_SIZE * 0.75;

            /* TODO: is this really needed?
            if(Config()->m_Debug)
            {
                char aBuf[256];
                str_format(aBuf, sizeof(aBuf), "shot player='%d:%s' team=%d weapon=%d", m_pPlayer->GetCID(), Server()->ClientName(m_pPlayer->GetCID()), m_pPlayer->GetTeam(), m_ActiveWeapon);
                GameServer()->Console()->Print(IConsole::OUTPUT_LEVEL_DEBUG, "game", aBuf);
            }*/

            // TODO: check all branches. make sure no code/TODO comments are in, before removing this comment
            match core.active_weapon {
                WeaponType::Hammer => {
                    // TODO: recheck
                    core.recoil_click = input.fire;
                    core.recoil_start_tick = cur_tick;
                    core.recoil_tick_amount = TICKS_UNTIL_RECOIL_ENDED;
                    self.entity_events
                        .push(CharacterEvent::Entity(EntityEvent::Sound {
                            pos: core.core.pos,
                            name: "weapons/hammer_fire".to_string(),
                        }));
                    // TODO: GameServer()->CreateSound(m_Pos, SOUND_HAMMER_FIRE);

                    let mut hits = 0;
                    let core_pos = core.core.pos;
                    pipe.characters.for_other_characters_in_range(
                        &proj_start_pos,
                        PHYSICAL_SIZE * 0.5,
                        &mut |char| {
                            let core_other = &char.core;
                            if pipe.collision.intersect_line(
                                &proj_start_pos,
                                &core_other.core.pos,
                                &mut vec2::default(),
                                &mut vec2::default(),
                            ) > 0
                            {
                                return;
                            }

                            // set his velocity to fast upward (for now)
                            if length(&(core_other.core.pos - proj_start_pos)) > 0.0 {
                                // TODO: GameServer()->CreateHammerHit(core_other.core.pos-normalize(&(core_other.core.pos-proj_start_pos))*GetProximityRadius()*0.5f);
                            } else {
                                // TODO: GameServer()->CreateHammerHit(proj_start_pos);
                            }

                            let Dir = if length(&(core_other.core.pos - core_pos)) > 0.0 {
                                normalize(&(core_other.core.pos - core_pos))
                            } else {
                                vec2::new(0.0, -1.0)
                            };

                            // TODO: Self::take_damage(characters, self_char_id, core_index, cur_tick, force, source, dmg_amount, from, weapon);
                            // TODO: pTarget->TakeDamage(vec2(0.f, -1.f) + normalize(Dir + vec2(0.f, -1.1f)) * 10.0f, Dir*-1, g_pData->m_Weapons.m_Hammer.m_pBase->m_Damage, m_pPlayer->GetCID(), m_ActiveWeapon);
                            hits += 1;
                        },
                    );
                    /* TODO:
                    // if we Hit anything, we have to wait for the reload
                    if(Hits)
                        m_ReloadTimer = Server()->TickSpeed()/3;*/
                }
                WeaponType::Gun => {
                    core.recoil_click = input.fire;
                    core.recoil_start_tick = cur_tick;
                    core.recoil_tick_amount = TICKS_UNTIL_RECOIL_ENDED;
                    self.entity_events.push(CharacterEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                        ty: WeaponType::Gun,
                    });
                    self.entity_events
                        .push(CharacterEvent::Entity(EntityEvent::Sound {
                            pos: core.core.pos,
                            name: "weapons/gun_fire".to_string(),
                        }));
                    /*new CProjectile(GameWorld(), WEAPON_GUN,
                        m_pPlayer->GetCID(),
                        ProjStartPos,
                        Direction,
                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_GunLifetime),
                        g_pData->m_Weapons.m_Gun.m_pBase->m_Damage, false, 0, -1, WEAPON_GUN);

                    GameServer()->CreateSound(m_Pos, SOUND_GUN_FIRE);*/
                }
                WeaponType::Shotgun => {
                    let shot_spreed: i32 = 2;

                    for i in -shot_spreed..=shot_spreed {
                        let spreading = [-0.185, -0.070, 0.0, 0.070, 0.185];
                        let a = angle(&direction) + spreading[(i + 2) as usize];
                        let _v = 1.0 - (i.abs() as f32 / (shot_spreed as f32));
                        let speed = 1.0; // TODO: mix((float)GameServer()->Tuning()->m_ShotgunSpeeddiff, 1.0f, v);

                        self.entity_events.push(CharacterEvent::Projectile {
                            pos: proj_start_pos,
                            dir: vec2::new(a.cos(), a.sin()) * speed,
                            ty: WeaponType::Shotgun,
                        });
                        /* TODO: new CProjectile(GameWorld(), WEAPON_SHOTGUN,
                        m_pPlayer->GetCID(),
                        ProjStartPos,
                        vec2(cosf(a), sinf(a))*Speed,
                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_ShotgunLifetime),
                        g_pData->m_Weapons.m_Shotgun.m_pBase->m_Damage, false, 0, -1, WEAPON_SHOTGUN);*/
                    }

                    self.entity_events
                        .push(CharacterEvent::Entity(EntityEvent::Sound {
                            pos: core.core.pos,
                            name: "weapons/shotgun_fire".to_string(),
                        }));
                }
                WeaponType::Grenade => {
                    core.recoil_click = input.fire;
                    core.recoil_start_tick = cur_tick;
                    core.recoil_tick_amount = TICKS_UNTIL_RECOIL_ENDED;
                    self.entity_events.push(CharacterEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                        ty: WeaponType::Grenade,
                    });
                    /*new CProjectile(GameWorld(), WEAPON_GRENADE,
                        m_pPlayer->GetCID(),
                        ProjStartPos,
                        Direction,
                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_GrenadeLifetime),
                        g_pData->m_Weapons.m_Grenade.m_pBase->m_Damage, true, 0, SOUND_GRENADE_EXPLODE, WEAPON_GRENADE);

                    GameServer()->CreateSound(m_Pos, SOUND_GRENADE_FIRE);*/
                }
                WeaponType::Laser => {
                    core.recoil_click = input.fire;
                    core.recoil_start_tick = cur_tick;
                    core.recoil_tick_amount = TICKS_UNTIL_RECOIL_ENDED;
                    self.entity_events.push(CharacterEvent::Laser {
                        pos: proj_start_pos,
                        dir: direction,
                    });
                    /*new CLaser(GameWorld(), m_Pos, Direction, GameServer()->Tuning()->m_LaserReach, m_pPlayer->GetCID());
                    GameServer()->CreateSound(m_Pos, SOUND_LASER_FIRE);*/
                }
                /*case WEAPON_NINJA:
                {
                    m_NumObjectsHit = 0;

                    m_Ninja.m_ActivationDir = Direction;
                    m_Ninja.m_CurrentMoveTime = g_pData->m_Weapons.m_Ninja.m_Movetime * Server()->TickSpeed() / 1000;
                    m_Ninja.m_OldVelAmount = length(m_Core.m_Vel);

                    GameServer()->CreateSound(m_Pos, SOUND_NINJA_FIRE);
                } break;*/
                _ => panic!("fire weapon is not implemented for this weapon"),
            }

            //core.m_AttackTick = cur_tick;

            //if(m_aWeapons[m_ActiveWeapon].m_Ammo > 0) // -1 == unlimited
            //    m_aWeapons[m_ActiveWeapon].m_Ammo--;

            //if(!m_ReloadTimer)
            //    m_ReloadTimer = g_pData->m_Weapons.m_aId[m_ActiveWeapon].m_Firedelay * Server()->TickSpeed() / 1000;
        }

        fn handle_weapon_switch(&mut self, cur_tick: GameTickType) {
            let wanted_weapon = if let Some(queued_weapon) = self.core.queued_weapon {
                queued_weapon
            } else {
                self.core.active_weapon
            };

            // select weapon
            let new_diff = *self.input.inp.weapon_diff;
            let old_diff = self.core.weapon_diff;
            self.core.weapon_diff = new_diff;
            let diff = new_diff.checked_sub(old_diff).unwrap_or(0);

            let cur_weapon_count = self.reusable_core.weapons.len();
            let offset = diff % cur_weapon_count as i32;

            let (found_weapon_index, _) = self
                .reusable_core
                .weapons
                .keys()
                .enumerate()
                .find(|(_, weapon)| (*weapon).eq(&wanted_weapon))
                .unwrap();

            // move the offset to where the actual weapon is
            let mut new_index = (found_weapon_index as i32 - offset) % cur_weapon_count as i32;
            if new_index < 0 {
                new_index += cur_weapon_count as i32;
            }

            let mut next_weapon = self
                .reusable_core
                .weapons
                .keys()
                .enumerate()
                .find_map(|(index, weapon)| {
                    if index == new_index as usize {
                        Some(*weapon)
                    } else {
                        None
                    }
                })
                .unwrap();

            // Direct Weapon selection
            if let Some(ref weapon) = *self.input.inp.weapon_req {
                if self.reusable_core.weapons.contains_key(weapon) {
                    next_weapon = *weapon;
                }
            }

            // check for insane values
            if next_weapon != self.core.active_weapon {
                self.core.queued_weapon = Some(next_weapon);
            }

            self.do_weapon_switch(cur_tick);
        }

        fn handle_weapons(&mut self, pipe: &mut SimulationPipeCharacter) {
            //ninja
            // TODO: HandleNinja();

            // check reload timer
            let core = &self.core;
            if pipe.cur_tick - core.recoil_start_tick <= core.recoil_tick_amount {
                return;
            }

            // fire Weapon, if wanted
            self.fire_weapon(pipe);

            // ammo regen
            /*int AmmoRegenTime = g_pData->m_Weapons.m_aId[m_ActiveWeapon].m_Ammoregentime;
            if(AmmoRegenTime && m_aWeapons[m_ActiveWeapon].m_Ammo >= 0)
            {
                // If equipped and not active, regen ammo?
                if(m_ReloadTimer <= 0)
                {
                    if(m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart < 0)
                        m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart = Server()->Tick();

                    if((Server()->Tick() - m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart) >= AmmoRegenTime * Server()->TickSpeed() / 1000)
                    {
                        // Add some ammo
                        m_aWeapons[m_ActiveWeapon].m_Ammo = minimum(m_aWeapons[m_ActiveWeapon].m_Ammo + 1,
                            g_pData->m_Weapons.m_aId[m_ActiveWeapon].m_Maxammo);
                        m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
                    }
                }
                else
                {
                    m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
                }
            }*/
        }
    }

    impl<'a> EntityInterface<CharacterCore, CharacterReusableCore, SimulationPipeCharacter<'a>>
        for Character
    {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipeCharacter) {}

        fn tick(&mut self, pipe: &mut SimulationPipeCharacter) {
            self.handle_weapon_switch(pipe.cur_tick);

            let mut core_pipe = CorePipe {
                characters: pipe.characters,
                input: &self.input,
                reusable_core: &mut self.reusable_core.core,
                character_id: &self.base.game_element_id,
            };
            self.core
                .core
                .physics_tick(true, true, &mut core_pipe, pipe.collision);

            if Entity::outside_of_playfield(&self.core.core.pos, pipe.collision) {
                self.die(pipe.cur_tick, None);
                return;
            }

            self.handle_weapons(pipe);
        }

        fn tick_deferred(&mut self, pipe: &mut SimulationPipeCharacter) {
            let mut core_pipe = CorePipe {
                characters: pipe.characters,
                input: &self.input,
                reusable_core: &mut self.reusable_core.core,
                character_id: &self.base.game_element_id,
            };
            self.core.core.physics_move(&mut core_pipe, &pipe.collision);
            self.core.core.physics_quantize();
        }

        fn split(&self) -> (&Entity, &CharacterCore, &Recycle<CharacterReusableCore>) {
            (&self.base, &self.core, &self.reusable_core)
        }

        fn split_mut(
            self: &mut Self,
        ) -> (
            &mut Entity,
            &mut CharacterCore,
            &mut PoolCharacterReusableCore,
        ) {
            (&mut self.base, &mut self.core, &mut self.reusable_core)
        }
    }

    pub type PoolCharacters = LinkedHashMap<TGameElementID, Character>;
    pub type Characters = PoolLinkedHashMap<TGameElementID, Character>;

    pub fn lerp_core_pos(char1: &Character, char2: &Character, amount: f64) -> vec2 {
        lerp(&char1.core.core.pos, &char2.core.core.pos, amount as f32)
    }

    pub fn lerp_core_vel(char1: &Character, char2: &Character, amount: f64) -> vec2 {
        lerp(&char1.core.core.vel, &char2.core.core.vel, amount as f32)
    }

    pub fn lerp_core_hook_pos(char1: &Character, char2: &Character, amount: f64) -> vec2 {
        lerp(
            &char1.core.core.hook_pos,
            &char2.core.core.hook_pos,
            amount as f32,
        )
    }
}
