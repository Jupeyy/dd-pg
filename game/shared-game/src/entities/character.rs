pub mod core;
pub mod hook;
pub mod player;
pub mod pos;

pub mod character {
    use std::{
        num::{NonZeroI64, NonZeroU64},
        sync::Arc,
    };

    use base::linked_hash_map_view::LinkedHashMapView;
    use base_log::log::SystemLog;
    use game_interface::{
        events::{
            GameBuffEvent, GameBuffNinjaEvent, GameBuffNinjaEventSound, GameCharacterEventEffect,
            GameCharacterEventSound,
        },
        types::{
            game::{
                GameEntityId, GameTickCooldown, GameTickCooldownAndLastActionCounter, GameTickType,
            },
            input::{CharacterInput, CharacterInputConsumableDiff, CharacterInputCursor},
            render::character::{CharacterBuff, CharacterDebuff, TeeEye},
            weapons::{WeaponType, NUM_WEAPONS},
        },
    };
    use hiarc::Hiarc;
    use shared_base::reusable::{CloneWithCopyableElements, ReusableCore};

    use super::{
        core::character_core::{Core, CorePipe, CoreReusable},
        hook::character_hook::{CharacterHook, Hook, HookedCharacters},
        player::player::{NoCharPlayer, NoCharPlayerType, NoCharPlayers, Players, PoolPlayerInfo},
        pos::character_pos::{CharacterPos, CharacterPositionPlayfield},
    };
    use crate::{
        collision::collision::Collision,
        entities::entity::entity::{Entity, EntityInterface, EntityTickResult},
        events::events::{CharacterDespawnInfo, CharacterDespawnType, CharacterEvent},
        simulation_pipe::simulation_pipe::{
            SimulationEntityEvents, SimulationEventWorldEntity, SimulationPipeCharacter,
        },
        state::state::TICKS_PER_SECOND,
        types::types::{GameOptions, GameTeam, GameType},
        weapons::definitions::weapon_def::Weapon,
    };

    use hashlink::{LinkedHashMap, LinkedHashSet};
    use math::math::{angle, distance_squared, length, lerp, mix, normalize, vector::vec2, PI};
    use pool::{datatypes::PoolLinkedHashMap, pool::Pool, recycle::Recycle, traits::Recyclable};
    use serde::{Deserialize, Serialize};

    use super::player::player::Player;

    pub const PHYSICAL_SIZE: f32 = 28.0;
    pub const TICKS_UNTIL_RECOIL_ENDED: GameTickType = 7;

    #[derive(Debug, Clone, Copy)]
    pub enum CharacterTeam {
        Red,
        Blue,
    }

    pub enum DamageTypes<'a> {
        Character(&'a GameEntityId),
        Team(CharacterTeam),
    }

    pub enum DamageBy {
        Ninja,
        Weapon(WeaponType),
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize, Copy, Clone)]
    pub struct BuffProps {
        pub remaining_tick: GameTickCooldown,
        pub interact_tick: GameTickCooldown,
        pub interact_cursor_dir: vec2,
        pub interact_val: f32,
    }

    #[derive(Debug, Hiarc, Default, Serialize, Deserialize, Copy, Clone)]
    pub struct CharacterCore {
        pub core: Core,
        // vanilla
        pub active_weapon: WeaponType,
        pub prev_weapon: WeaponType,
        pub queued_weapon: Option<WeaponType>,
        pub health: u32,
        pub armor: u32,
        pub attack_recoil: GameTickCooldownAndLastActionCounter,
        pub no_ammo_sound: GameTickCooldown,

        pub score: i64,
        pub team: Option<GameTeam>,

        pub eye: TeeEye,
        pub normal_eye_in: GameTickCooldown,

        pub(crate) input: CharacterInput,

        /// How long is the player in this game.
        /// On new round begin this should be resetted.
        pub game_ticks_passed: GameTickType,

        /// How many ticks passed since the animation started.
        pub animation_ticks_passed: GameTickType,

        /// is timeout e.g. by a network disconnect.
        /// this is a hint, not a logic variable.
        pub is_timeout: bool,
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone)]
    pub struct CharacterReusableCore {
        pub core: CoreReusable,
        pub weapons: LinkedHashMap<WeaponType, Weapon>,
        pub buffs: LinkedHashMap<CharacterBuff, BuffProps>,
        pub debuffs: LinkedHashMap<CharacterDebuff, BuffProps>,

        pub interactions: LinkedHashSet<GameEntityId>,
    }

    impl CloneWithCopyableElements for CharacterReusableCore {
        fn copy_clone_from(&mut self, other: &Self) {
            self.core.copy_clone_from(&other.core);
            self.weapons.copy_clone_from(&other.weapons);
            self.buffs.copy_clone_from(&other.buffs);
            self.debuffs.copy_clone_from(&other.debuffs);
            self.interactions.clone_from(&other.interactions);
        }
    }

    impl Recyclable for CharacterReusableCore {
        fn new() -> Self {
            Self {
                core: CoreReusable::new(),
                weapons: Default::default(),
                buffs: Default::default(),
                debuffs: Default::default(),
                interactions: Default::default(),
            }
        }
        fn reset(&mut self) {
            self.core.reset();
            self.weapons.reset();
            self.buffs.reset();
            self.debuffs.reset();
            self.interactions.reset();
        }
    }

    impl ReusableCore for CharacterReusableCore {}

    pub type PoolCharacterReusableCore = Recycle<CharacterReusableCore>;

    #[derive(Debug, Hiarc, Clone)]
    pub struct CharacterPool {
        pub(crate) character_pool: Pool<PoolCharacters>,
        pub(crate) character_reusable_cores_pool: Pool<CharacterReusableCore>,
    }

    #[derive(Debug, Hiarc, PartialEq, Eq)]
    pub enum CharacterDamageResult {
        None,
        Damage,
        Death,
    }

    #[derive(Debug, Hiarc)]
    pub enum CharacterPlayerTy {
        /// e.g. server side dummy
        None,
        /// usually a normal human player
        Player {
            /// keep a reference to the players, the client automatically deletes the player if
            /// it is destroyed
            players: Players,
            /// same as `players`
            no_char_players: NoCharPlayers,
        },
    }

    #[derive(Debug, Hiarc)]
    pub struct Character {
        pub(crate) base: Entity,
        pub(crate) core: CharacterCore,
        pub(crate) reusable_core: PoolCharacterReusableCore,
        pub(crate) player_info: PoolPlayerInfo,
        pub(crate) pos: CharacterPos,
        pub(crate) hook: CharacterHook,

        pub(crate) entity_events: Vec<CharacterEvent>,
        simulation_events: SimulationEntityEvents,
        despawn_info: CharacterDespawnType,

        ty: CharacterPlayerTy,
    }

    impl Character {
        pub fn new(
            id: &GameEntityId,
            log: &Arc<SystemLog>,
            character_pool: &CharacterPool,
            player_info: PoolPlayerInfo,
            player_input: CharacterInput,
            simulation_events: &SimulationEntityEvents,
            stage_id: &GameEntityId,
            ty: CharacterPlayerTy,
            pos: vec2,
            field: &CharacterPositionPlayfield,
            hooks: &HookedCharacters,
            team: Option<GameTeam>,
        ) -> Self {
            let mut core = CharacterCore::default();
            core.team = team;
            core.health = 10;
            core.armor = 0;
            core.input = player_input;

            if let CharacterPlayerTy::Player { players, .. } = &ty {
                players.insert(
                    *id,
                    Player {
                        stage_id: *stage_id,
                    },
                );
            }

            simulation_events.push(SimulationEventWorldEntity::Character {
                player_id: *id,
                ev: CharacterEvent::Effect {
                    pos: pos / 32.0,
                    ev: GameCharacterEventEffect::Spawn,
                },
            });
            simulation_events.push(SimulationEventWorldEntity::Character {
                player_id: *id,
                ev: CharacterEvent::Sound {
                    pos: pos / 32.0,
                    ev: GameCharacterEventSound::Spawn,
                },
            });

            let reusable_core = character_pool.character_reusable_cores_pool.new();

            Self {
                base: Entity::new(id, log.logger("character")),
                core,
                reusable_core,
                player_info,
                pos: field.get_character_pos(pos, *id),
                hook: hooks.get_new_hook(*id),

                entity_events: Default::default(),
                simulation_events: simulation_events.clone(),
                despawn_info: Default::default(),

                ty,
            }
        }

        pub(crate) fn is_player_character(&self) -> bool {
            matches!(self.ty, CharacterPlayerTy::Player { .. })
        }

        pub(crate) fn die(&mut self, killer_id: Option<GameEntityId>) {
            self.despawn_info = CharacterDespawnType::Default(CharacterDespawnInfo {
                pos: *self.pos.pos(),
                respawns_in_ticks: (TICKS_PER_SECOND / 2).into(),
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

        /// sets the despawn info to a silently drop the player from the game
        /// it won't be added to the spectators etc.
        /// pending simulation events are still processed.
        pub fn despawn_completely_silent(&mut self) {
            self.despawn_info = CharacterDespawnType::DropFromGame;
        }

        /// the user wants to respawn (a.k.a. kill)
        pub fn despawn_to_respawn(&mut self) {
            self.despawn_info = CharacterDespawnType::Default(CharacterDespawnInfo {
                pos: *self.pos.pos(),
                respawns_in_ticks: 1.into(),
                killer_id: None,
            });
        }

        /// normally only useful for snapshot
        pub fn update_player_ty(&mut self, stage_id: &GameEntityId, player_ty: CharacterPlayerTy) {
            match &self.ty {
                CharacterPlayerTy::None => {
                    if let CharacterPlayerTy::Player { players, .. } = &player_ty {
                        players.insert(
                            self.base.game_element_id,
                            Player {
                                stage_id: *stage_id,
                            },
                        );
                        self.ty = player_ty;
                    }
                }
                CharacterPlayerTy::Player { players, .. } => {
                    if let CharacterPlayerTy::None = &player_ty {
                        players.remove(&self.base.game_element_id);
                        self.ty = player_ty;
                    }
                }
            }
        }

        pub fn give_ninja(&mut self) {
            self.reusable_core.buffs.insert(
                CharacterBuff::Ninja,
                BuffProps {
                    remaining_tick: (15 * TICKS_PER_SECOND).into(),
                    interact_tick: 0.into(),
                    interact_cursor_dir: vec2::default(),
                    interact_val: 0.0,
                },
            );
            self.core
                .attack_recoil
                .advance_ticks_passed_to_cooldown_len();
        }

        fn set_weapon(&mut self, new_weapon: WeaponType) {
            if self.core.active_weapon == new_weapon {
                return;
            }

            self.core.prev_weapon = self.core.active_weapon;
            self.core.queued_weapon = None;
            self.core.active_weapon = new_weapon;
            self.entity_events.push(CharacterEvent::Sound {
                pos: *self.pos.pos() / 32.0,
                ev: GameCharacterEventSound::WeaponSwitch { new_weapon },
            });

            if self.core.active_weapon as usize >= NUM_WEAPONS {
                self.core.active_weapon = Default::default(); // TODO: what is the idea behind this?
            }
            if let Some(_) = self.reusable_core.weapons.get_mut(&self.core.active_weapon) {
                // TODO: weapon.next_ammo_regeneration_tick
                //core.weapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
            }
        }

        fn do_weapon_switch(&mut self) {
            // make sure we can switch
            if self.core.attack_recoil.is_some() || self.core.queued_weapon.is_none() {
                return;
            }

            // switch weapon
            self.set_weapon(self.core.queued_weapon.unwrap());
        }

        pub fn is_friendly_fire(
            _characters: &mut Characters,
            _self_char_id: &GameEntityId,
            _other_char_id: &GameEntityId,
        ) -> bool {
            false // TODO
        }

        pub fn is_friendly_fire_team(
            _characters: &mut Characters,
            _self_char_id: &GameEntityId,
            _other_team: CharacterTeam,
        ) -> bool {
            false // TODO
        }

        fn create_damage_indicators(
            entity_events: &mut Vec<CharacterEvent>,
            pos: &vec2,
            angle: f32,
            amount: usize,
        ) {
            let a = 3.0 * PI / 2.0 + angle;
            let s = a - PI / 3.0;
            let e = a + PI / 3.0;
            for i in 0..amount {
                let f = mix(&s, &e, (i + 1) as f32 / (amount + 2) as f32);

                let angle = f;
                let dir = vec2::new(angle.cos(), angle.sin()) * -75.0 / 4.0;
                entity_events.push(CharacterEvent::Effect {
                    pos: *pos / 32.0,
                    ev: GameCharacterEventEffect::DamageIndicator {
                        pos: *pos / 32.0,
                        vel: dir,
                    },
                });
            }
        }

        fn create_hammer_hit(entity_events: &mut Vec<CharacterEvent>, char_pos: &vec2, pos: &vec2) {
            entity_events.push(CharacterEvent::Effect {
                pos: *char_pos / 32.0,
                ev: GameCharacterEventEffect::HammerHit { pos: *pos / 32.0 },
            });
            entity_events.push(CharacterEvent::Sound {
                pos: *char_pos / 32.0,
                ev: GameCharacterEventSound::HammerHit { pos: *pos / 32.0 },
            });
        }

        pub fn take_damage_from(
            self_char: &mut Character,
            self_char_id: &GameEntityId,
            killer_id: Option<GameEntityId>,
            force: &vec2,
            _source: &vec2,
            mut dmg_amount: u32,
            from: DamageTypes,
            _by: DamageBy,
        ) -> CharacterDamageResult {
            let core = &mut self_char.core;
            core.core.vel += *force;
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

            // TODO: old heath - heath + old armor - armor
            Self::create_damage_indicators(
                &mut self_char.entity_events,
                self_char.pos.pos(),
                0.0,
                dmg_amount as usize,
            );
            if let DamageTypes::Character(id) = &from {
                if *id != self_char_id {
                    self_char.entity_events.push(CharacterEvent::Sound {
                        pos: *self_char.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::Hit { strong: false },
                    });
                }
            }

            // check for death
            if core.health == 0 {
                self_char.die(killer_id);
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

                return CharacterDamageResult::Death;
            }

            self_char.entity_events.push(CharacterEvent::Sound {
                pos: *self_char.pos.pos() / 32.0,
                ev: GameCharacterEventSound::Pain {
                    long: dmg_amount > 2,
                },
            });

            core.normal_eye_in = (TICKS_PER_SECOND / 2).into();
            core.eye = TeeEye::Pain;
            /* TODO:

            return true;*/
            CharacterDamageResult::Damage
        }

        pub fn take_damage(
            characters: &mut Characters,
            self_char_id: &GameEntityId,
            force: &vec2,
            source: &vec2,
            mut dmg_amount: u32,
            from: DamageTypes,
            by: DamageBy,
        ) -> CharacterDamageResult {
            let killer_id = match &from {
                DamageTypes::Character(&from_id) => {
                    if Self::is_friendly_fire(characters, self_char_id, &from_id) {
                        return CharacterDamageResult::None;
                    }

                    // m_pPlayer only inflicts half damage on self
                    if from_id == *self_char_id {
                        dmg_amount = 1.max(dmg_amount / 2);
                    }
                    Some(from_id)
                }
                DamageTypes::Team(team) => {
                    if Self::is_friendly_fire_team(characters, self_char_id, *team) {
                        return CharacterDamageResult::None;
                    }
                    None
                }
            };

            let self_char = characters.get_mut(self_char_id).unwrap();
            let res = Self::take_damage_from(
                self_char,
                self_char_id,
                killer_id,
                force,
                source,
                dmg_amount,
                from,
                by,
            );
            if let (CharacterDamageResult::Death, Some(killer)) =
                (&res, killer_id.map(|id| characters.get_mut(&id)).flatten())
            {
                killer.core.eye = TeeEye::Happy;
                killer.core.normal_eye_in = (TICKS_PER_SECOND / 2).into();
            }
            res
        }

        /// can fire at all (ninja or weapon)
        fn can_fire(&self) -> bool {
            !self.reusable_core.buffs.contains_key(&CharacterBuff::Ghost)
                && !self
                    .reusable_core
                    .debuffs
                    .contains_key(&CharacterDebuff::Freeze)
        }

        fn can_fire_weapon(&self) -> bool {
            !self.reusable_core.buffs.contains_key(&CharacterBuff::Ninja) && self.can_fire()
        }

        fn fire_weapon(
            &mut self,
            pipe: &mut SimulationPipeCharacter,
            fire: Option<(NonZeroU64, CharacterInputCursor)>,
        ) {
            if self.core.attack_recoil.is_some() {
                return;
            }

            self.do_weapon_switch();

            if !self.can_fire_weapon() {
                return;
            }

            let core = &mut self.core;
            let input = &core.input;

            let full_auto = if core.active_weapon == WeaponType::Grenade
                || core.active_weapon == WeaponType::Shotgun
                || core.active_weapon == WeaponType::Laser
            {
                true
            } else {
                false
            };

            let auto_fired = full_auto && *input.state.fire;
            let fired = fire.is_some();

            let direction = normalize(&{
                let cursor_pos = if fired {
                    fire.as_ref().unwrap().1.to_vec2()
                } else {
                    input.cursor.to_vec2()
                };
                vec2::new(cursor_pos.x as f32, cursor_pos.y as f32)
            });

            // check if we gonna fire
            let will_fire = fired || auto_fired;

            if !will_fire {
                return;
            }

            // check for ammo
            let cur_weapon = self.reusable_core.weapons.get_mut(&core.active_weapon);
            if !cur_weapon
                .as_ref()
                .is_some_and(|weapon| !weapon.cur_ammo.is_some_and(|val| val == 0))
            {
                if fired && core.no_ammo_sound.is_none() {
                    self.entity_events.push(CharacterEvent::Sound {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::NoAmmo {
                            weapon: core.active_weapon,
                        },
                    });
                    core.no_ammo_sound = TICKS_PER_SECOND.into();
                }
                return;
            }

            let cur_weapon = cur_weapon.unwrap();
            let proj_start_pos = *self.pos.pos() + direction * PHYSICAL_SIZE * 0.75;

            // TODO: check all branches. make sure no code/TODO comments are in, before removing this comment

            core.attack_recoil = match core.active_weapon {
                WeaponType::Hammer => {
                    // TODO: recheck
                    self.entity_events.push(CharacterEvent::Sound {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::HammerFire,
                    });

                    let mut hits = 0;
                    let core_pos = *self.pos.pos();
                    pipe.characters.for_other_characters_in_range(
                        &proj_start_pos,
                        PHYSICAL_SIZE * 0.5,
                        &mut |char, removed_characters| {
                            if pipe.collision.intersect_line(
                                &proj_start_pos,
                                char.pos.pos(),
                                &mut vec2::default(),
                                &mut vec2::default(),
                            ) > 0
                            {
                                return;
                            }

                            // set his velocity to fast upward (for now)
                            if length(&(*char.pos.pos() - proj_start_pos)) > 0.0 {
                                Self::create_hammer_hit(
                                    &mut self.entity_events,
                                    self.pos.pos(),
                                    &(*char.pos.pos()
                                        - normalize(&(*char.pos.pos() - proj_start_pos))
                                            * PHYSICAL_SIZE
                                            * 0.5),
                                );
                            } else {
                                Self::create_hammer_hit(
                                    &mut self.entity_events,
                                    self.pos.pos(),
                                    &proj_start_pos,
                                );
                            }

                            let dir = if length(&(*char.pos.pos() - core_pos)) > 0.0 {
                                normalize(&(*char.pos.pos() - core_pos))
                            } else {
                                vec2::new(0.0, -1.0)
                            };

                            let char_id = char.base.game_element_id;
                            if Self::take_damage_from(
                                char,
                                &char_id,
                                Some(self.base.game_element_id),
                                &(vec2::new(0.0, -1.0)
                                    + normalize(&(dir + vec2::new(0.0, -1.1))) * 10.0),
                                &(dir * -1.0),
                                3,
                                DamageTypes::Character(&self.base.game_element_id),
                                DamageBy::Weapon(WeaponType::Hammer),
                            ) == CharacterDamageResult::Death
                            {
                                removed_characters.insert(char.base.game_element_id);

                                core.eye = TeeEye::Happy;
                                core.normal_eye_in = (TICKS_PER_SECOND / 2).into();
                            }
                            hits += 1;
                        },
                    );
                    if hits > 0 {
                        let fire_delay = pipe
                            .collision
                            .get_tune_at(&proj_start_pos)
                            .hammer_fire_delay;
                        ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType)
                            .into()
                    } else {
                        1.into()
                    }
                }
                WeaponType::Gun => {
                    let tunings = pipe.collision.get_tune_at(&proj_start_pos);
                    self.entity_events.push(CharacterEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                        ty: WeaponType::Gun,
                        lifetime: tunings.gun_lifetime,
                    });
                    self.entity_events.push(CharacterEvent::Sound {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::GunFire,
                    });
                    /*new CProjectile(GameWorld(), WEAPON_GUN,
                                        m_pPlayer->GetCID(),
                                        ProjStartPos,
                                        Direction,
                                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_GunLifetime),
                                        g_pData->m_Weapons.m_Gun.m_pBase->m_Damage, false, 0, -1, WEAPON_GUN);
                    */

                    let fire_delay = tunings.gun_fire_delay;
                    ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into()
                }
                WeaponType::Shotgun => {
                    let shot_spreed: i32 = 2;

                    for i in -shot_spreed..=shot_spreed {
                        let spreading = [-0.185, -0.070, 0.0, 0.070, 0.185];
                        let a = angle(&direction) + spreading[(i + 2) as usize];
                        let v = 1.0 - (i.abs() as f32 / (shot_spreed as f32));
                        let tunings = pipe.collision.get_tune_at(&proj_start_pos);
                        let speed = mix(&tunings.shotgun_speeddiff, &1.0, v);

                        self.entity_events.push(CharacterEvent::Projectile {
                            pos: proj_start_pos,
                            dir: vec2::new(a.cos(), a.sin()) * speed,
                            ty: WeaponType::Shotgun,
                            lifetime: tunings.shotgun_lifetime,
                        });
                        /* TODO: new CProjectile(GameWorld(), WEAPON_SHOTGUN,
                        m_pPlayer->GetCID(),
                        ProjStartPos,
                        vec2(cosf(a), sinf(a))*Speed,
                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_ShotgunLifetime),
                        g_pData->m_Weapons.m_Shotgun.m_pBase->m_Damage, false, 0, -1, WEAPON_SHOTGUN);*/
                    }

                    self.entity_events.push(CharacterEvent::Sound {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::ShotgunFire,
                    });

                    let fire_delay = pipe
                        .collision
                        .get_tune_at(&proj_start_pos)
                        .shotgun_fire_delay;
                    ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into()
                }
                WeaponType::Grenade => {
                    let tunings = pipe.collision.get_tune_at(&proj_start_pos);
                    self.entity_events.push(CharacterEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                        ty: WeaponType::Grenade,
                        lifetime: tunings.grenade_lifetime,
                    });
                    self.entity_events.push(CharacterEvent::Sound {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::GrenadeFire,
                    });
                    /*new CProjectile(GameWorld(), WEAPON_GRENADE,
                                        m_pPlayer->GetCID(),
                                        ProjStartPos,
                                        Direction,
                                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_GrenadeLifetime),
                                        g_pData->m_Weapons.m_Grenade.m_pBase->m_Damage, true, 0, SOUND_GRENADE_EXPLODE, WEAPON_GRENADE);
                    */
                    let fire_delay = tunings.grenade_fire_delay;
                    ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into()
                }
                WeaponType::Laser => {
                    self.entity_events.push(CharacterEvent::Laser {
                        pos: *self.pos.pos(),
                        dir: direction,
                        energy: pipe.collision.get_tune_at(self.pos.pos()).laser_reach,
                    });
                    self.entity_events.push(CharacterEvent::Sound {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameCharacterEventSound::LaserFire,
                    });

                    let fire_delay = pipe.collision.get_tune_at(&proj_start_pos).laser_fire_delay;
                    ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into()
                }
            };

            cur_weapon.cur_ammo = cur_weapon.cur_ammo.map(|val| val.saturating_sub(1));
        }

        fn fire_ninja(
            &mut self,
            fire: &Option<(NonZeroU64, CharacterInputCursor)>,
            collision: &Collision,
        ) {
            if !self.can_fire() {
                return;
            }
            if self.core.attack_recoil.is_some() {
                return;
            }
            let Some((_, cursor)) = fire else { return };
            let Some(buff) = self.reusable_core.buffs.get_mut(&CharacterBuff::Ninja) else {
                return;
            };

            let fire_delay = collision.get_tune_at(self.pos.pos()).ninja_fire_delay;
            self.core.attack_recoil =
                ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into();

            let cursor = cursor.to_vec2();
            buff.interact_cursor_dir = normalize(&vec2::new(cursor.x as f32, cursor.y as f32));
            buff.interact_tick = (TICKS_PER_SECOND / 5).into();
            buff.interact_val = length(&self.core.core.vel);
            self.reusable_core.interactions.clear();

            self.simulation_events
                .push(SimulationEventWorldEntity::Character {
                    player_id: self.base.game_element_id,
                    ev: CharacterEvent::Buff {
                        pos: *self.pos.pos() / 32.0,
                        ev: GameBuffEvent::Ninja(GameBuffNinjaEvent::Sound(
                            GameBuffNinjaEventSound::Attack,
                        )),
                    },
                });
        }

        fn handle_weapon_switch(
            &mut self,
            weapon_diff: Option<NonZeroI64>,
            weapon_req: Option<WeaponType>,
        ) {
            let wanted_weapon = if let Some(queued_weapon) = self.core.queued_weapon {
                queued_weapon
            } else {
                self.core.active_weapon
            };

            // select weapon
            let diff = weapon_diff.map(|diff| diff.get()).unwrap_or(0);

            let cur_weapon_count = self.reusable_core.weapons.len();
            let offset = diff as i32 % cur_weapon_count as i32;

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
            if let Some(ref weapon) = weapon_req {
                if self.reusable_core.weapons.contains_key(weapon) {
                    next_weapon = *weapon;
                }
            }

            // check for insane values
            if next_weapon != self.core.active_weapon {
                self.core.queued_weapon = Some(next_weapon);
            }

            self.do_weapon_switch();
        }

        fn handle_buffs_and_debuffs(&mut self, pipe: &mut SimulationPipeCharacter) {
            self.reusable_core.buffs.retain_with_order(|ty, buff| {
                if buff.remaining_tick.tick().unwrap_or_default()
                    && matches!(ty, CharacterBuff::Ninja)
                {
                    self.core
                        .attack_recoil
                        .advance_ticks_passed_to_cooldown_len();
                }
                buff.remaining_tick.is_some()
            });

            self.handle_ninja(pipe);
        }

        fn handle_ninja(&mut self, pipe: &mut SimulationPipeCharacter) {
            let Some(buff) = self.reusable_core.buffs.get_mut(&CharacterBuff::Ninja) else {
                return;
            };
            if buff.interact_tick.is_none() {
                return;
            }
            if buff.interact_tick.tick().unwrap_or_default() {
                self.core.core.vel = buff.interact_cursor_dir * buff.interact_val;
            } else {
                // Set velocity
                let mut vel = buff.interact_cursor_dir * 50.0;
                let old_pos = *self.pos.pos();
                let mut new_pos = *self.pos.pos();
                pipe.collision.move_box(
                    &mut new_pos,
                    &mut vel,
                    &vec2::new(PHYSICAL_SIZE, PHYSICAL_SIZE),
                    0.0,
                );
                self.pos.move_pos(new_pos);

                self.core.core.vel = vec2::new(0.0, 0.0);

                let dir = *self.pos.pos() - old_pos;
                let center = old_pos + dir * 0.5;
                pipe.characters.for_other_characters_in_range(
                    &center,
                    PHYSICAL_SIZE * 2.0,
                    &mut |char, removed_chars| {
                        let char_id = char.base.game_element_id;
                        // make sure we haven't Hit this object before
                        if self.reusable_core.interactions.contains(&char_id) {
                            return;
                        }

                        // check so we are sufficiently close
                        if distance_squared(char.pos.pos(), self.pos.pos())
                            > (PHYSICAL_SIZE * 2.0).powf(2.0)
                        {
                            return;
                        }

                        self.reusable_core.interactions.insert(char_id);

                        if Self::take_damage_from(
                            char,
                            &char_id,
                            Some(self.base.game_element_id),
                            &vec2::new(0.0, -10.0),
                            self.pos.pos(),
                            9,
                            DamageTypes::Character(&self.base.game_element_id),
                            DamageBy::Ninja,
                        ) == CharacterDamageResult::Death
                        {
                            removed_chars.insert(char_id);

                            self.core.eye = TeeEye::Happy;
                            self.core.normal_eye_in = (TICKS_PER_SECOND / 2).into();
                        }

                        self.simulation_events
                            .push(SimulationEventWorldEntity::Character {
                                player_id: self.base.game_element_id,
                                ev: CharacterEvent::Buff {
                                    pos: *self.pos.pos() / 32.0,
                                    ev: GameBuffEvent::Ninja(GameBuffNinjaEvent::Sound(
                                        GameBuffNinjaEventSound::Hit,
                                    )),
                                },
                            });
                    },
                );
            }
        }

        fn handle_weapons(&mut self, pipe: &mut SimulationPipeCharacter) {
            // don't handle weapon if ninja, ghost or freeze are active
            if self.reusable_core.buffs.contains_key(&CharacterBuff::Ninja)
                || self.reusable_core.buffs.contains_key(&CharacterBuff::Ghost)
                || self
                    .reusable_core
                    .debuffs
                    .contains_key(&CharacterDebuff::Freeze)
            {
                return;
            }

            // check reload timer
            if self.core.attack_recoil.is_some() {
                return;
            }

            // fire weapon, if wanted
            self.fire_weapon(pipe, None);

            // ammo regen
            let ammo_regen_time = match self.core.active_weapon {
                WeaponType::Hammer => None,
                WeaponType::Gun => Some(TICKS_PER_SECOND / 2),
                WeaponType::Shotgun => None,
                WeaponType::Grenade => None,
                WeaponType::Laser => None,
            };
            let weapon = self
                .reusable_core
                .weapons
                .get_mut(&self.core.active_weapon)
                .unwrap();
            if let Some(ammo_regen_time) = ammo_regen_time {
                if weapon.cur_ammo.is_some_and(|ammo| ammo >= 10) {
                    weapon.next_ammo_regeneration_tick = ammo_regen_time.into();
                } else if weapon
                    .next_ammo_regeneration_tick
                    .tick()
                    .unwrap_or_default()
                {
                    weapon.cur_ammo = weapon.cur_ammo.map(|ammo| ammo + 1);
                    weapon.next_ammo_regeneration_tick = ammo_regen_time.into();
                }
            }
        }

        pub fn handle_input_change(
            &mut self,
            pipe: &mut SimulationPipeCharacter,
            diff: CharacterInputConsumableDiff,
        ) -> EntityTickResult {
            self.core.core.queued_jumps = self
                .core
                .core
                .queued_jumps
                .saturating_add(diff.jump.map(|val| val.get()).unwrap_or_default());
            if let Some((hooks, cursor)) = diff.hook {
                self.core.core.queued_hooks.clicked = self
                    .core
                    .core
                    .queued_hooks
                    .clicked
                    .saturating_add(hooks.get());
                self.core.core.queued_hooks.cursor = cursor.to_vec2();
            }
            self.handle_weapon_switch(diff.weapon_diff, diff.weapon_req);
            self.fire_ninja(&diff.fire, pipe.collision);
            self.fire_weapon(pipe, diff.fire);
            EntityTickResult::None
        }

        fn handle_ticks(&mut self) {
            self.core.attack_recoil.tick();
            self.core.no_ammo_sound.tick();

            self.core.animation_ticks_passed += 1;
            self.core.game_ticks_passed += 1;
        }
    }

    impl<'a> EntityInterface<CharacterCore, CharacterReusableCore, SimulationPipeCharacter<'a>>
        for Character
    {
        fn pre_tick(&mut self, _pipe: &mut SimulationPipeCharacter) -> EntityTickResult {
            if self.core.normal_eye_in.tick().unwrap_or_default() {
                self.core.eye = TeeEye::Normal;
            }

            EntityTickResult::None
        }

        fn tick(&mut self, pipe: &mut SimulationPipeCharacter) -> EntityTickResult {
            self.handle_ticks();

            self.handle_weapon_switch(None, None);

            let mut core_pipe = CorePipe {
                characters: pipe.characters,
                input: &self.core.input,
                reusable_core: &mut self.reusable_core.core,
                character_id: &self.base.game_element_id,
            };
            self.core.core.physics_tick(
                &mut self.pos,
                &mut self.hook,
                true,
                true,
                &mut core_pipe,
                pipe.collision,
                &mut self.entity_events,
            );

            if Entity::outside_of_playfield(self.pos.pos(), pipe.collision) {
                self.die(None);
                return EntityTickResult::RemoveEntity;
            }

            self.handle_buffs_and_debuffs(pipe);
            self.handle_weapons(pipe);

            EntityTickResult::None
        }

        fn tick_deferred(&mut self, pipe: &mut SimulationPipeCharacter) -> EntityTickResult {
            let mut core_pipe = CorePipe {
                characters: pipe.characters,
                input: &self.core.input,
                reusable_core: &mut self.reusable_core.core,
                character_id: &self.base.game_element_id,
            };
            self.core
                .core
                .physics_move(&mut self.pos, &mut core_pipe, &pipe.collision);
            self.core
                .core
                .physics_quantize(&mut self.pos, &mut self.hook);

            EntityTickResult::None
        }

        fn drop_silent(&mut self) {
            self.base.drop_silent = true;
        }
    }

    impl Drop for Character {
        fn drop(&mut self) {
            if !self.base.drop_silent {
                for ev in self.entity_events.drain(..) {
                    self.simulation_events
                        .push(SimulationEventWorldEntity::Character {
                            player_id: self.base.game_element_id,
                            ev,
                        });
                }
            }

            let (is_dead, add_to_no_char_players, death_effect) = match &mut self.despawn_info {
                CharacterDespawnType::Default(despawn_info) => {
                    let is_dead = despawn_info.respawns_in_ticks.is_some();
                    if !self.base.drop_silent {
                        self.simulation_events
                            .push(SimulationEventWorldEntity::Character {
                                player_id: self.base.game_element_id,
                                ev: CharacterEvent::Despawn {
                                    killer_id: despawn_info.killer_id,
                                },
                            });
                    }
                    (is_dead, true, true)
                }
                CharacterDespawnType::DropFromGame => (false, false, true),
            };

            let (is_dead, add_to_no_char_players, death_effect) = (
                is_dead && !self.base.drop_silent,
                add_to_no_char_players && !self.base.drop_silent,
                death_effect && !self.base.drop_silent,
            );

            if death_effect {
                self.simulation_events
                    .push(SimulationEventWorldEntity::Character {
                        player_id: self.base.game_element_id,
                        ev: CharacterEvent::Effect {
                            pos: *self.pos.pos() / 32.0,
                            ev: GameCharacterEventEffect::Death,
                        },
                    });
                self.simulation_events
                    .push(SimulationEventWorldEntity::Character {
                        player_id: self.base.game_element_id,
                        ev: CharacterEvent::Sound {
                            pos: *self.pos.pos() / 32.0,
                            ev: GameCharacterEventSound::Death,
                        },
                    });
            }

            if let CharacterPlayerTy::Player {
                players,
                no_char_players,
            } = &self.ty
            {
                players.remove(&self.base.game_element_id);
                if add_to_no_char_players {
                    no_char_players.insert(
                        self.base.game_element_id,
                        NoCharPlayer::new(
                            self.player_info.take_by_item_without_pool(),
                            self.core.input.clone(),
                            &self.base.game_element_id,
                            if is_dead {
                                NoCharPlayerType::Dead {
                                    respawn_in_ticks: if let CharacterDespawnType::Default(
                                        despawn_info,
                                    ) = &self.despawn_info
                                    {
                                        despawn_info.respawns_in_ticks
                                    } else {
                                        0.into()
                                    },
                                }
                            } else {
                                NoCharPlayerType::Spectator
                            },
                        ),
                    );
                }
            }
        }
    }

    pub type PoolCharacters = LinkedHashMap<GameEntityId, Character>;

    pub type CharactersView<'a, F> = LinkedHashMapView<'a, GameEntityId, Character, F>;

    pub type Characters = PoolLinkedHashMap<GameEntityId, Character>;

    pub fn lerp_core_pos(char1: &Character, char2: &Character, amount: f64) -> vec2 {
        lerp(char1.pos.pos(), char2.pos.pos(), amount as f32)
    }

    pub fn lerp_core_vel(char1: &Character, char2: &Character, amount: f64) -> vec2 {
        lerp(&char1.core.core.vel, &char2.core.core.vel, amount as f32)
    }

    pub fn lerp_core_hook_pos(char1: &Character, char2: &Character, amount: f64) -> Option<vec2> {
        if let (Hook::Active { hook_pos: pos1, .. }, Hook::Active { hook_pos: pos2, .. }) =
            (char1.hook.hook(), char2.hook.hook())
        {
            Some(lerp(&pos1, &pos2, amount as f32))
        } else {
            None
        }
    }
}
