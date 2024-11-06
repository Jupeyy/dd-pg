pub mod core;
pub mod hook;
pub mod player;
pub mod pos;

pub mod character {
    use std::num::{NonZeroI64, NonZeroU64};

    use base::linked_hash_map_view::LinkedHashMapView;
    use game_interface::{
        events::{
            GameBuffNinjaEventSound, GameBuffSoundEvent, GameCharacterEventEffect,
            GameCharacterEventSound, GameWorldActionKillWeapon,
        },
        types::{
            emoticons::{EmoticonType, EnumCount},
            game::{GameTickCooldown, GameTickCooldownAndLastActionCounter, GameTickType},
            id_types::{CharacterId, StageId},
            input::{CharacterInput, CharacterInputConsumableDiff, CharacterInputCursor},
            network_stats::PlayerNetworkStats,
            render::{
                character::{CharacterBuff, CharacterDebuff, TeeEye},
                game::game_match::MatchSide,
            },
            weapons::WeaponType,
        },
    };
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use shared_base::{
        mapdef_06::DdraceTileNum,
        reusable::{CloneWithCopyableElements, ReusableCore},
    };

    use super::{
        core::character_core::{Core, CoreEvents, CorePipe, CoreReusable, PHYSICAL_SIZE},
        hook::character_hook::{CharacterHook, Hook, HookedCharacters},
        player::player::{NoCharPlayer, NoCharPlayerType, NoCharPlayers, PlayerInfo, Players},
        pos::character_pos::{CharacterPos, CharacterPositionPlayfield},
    };
    use crate::{
        collision::collision::{Collision, CollisionTile, CollisionTypes, HitTile},
        entities::entity::entity::{DropMode, Entity, EntityInterface, EntityTickResult},
        events::events::{
            CharacterDespawnInfo, CharacterDespawnType, CharacterEvent, CharacterTickEvent,
        },
        simulation_pipe::simulation_pipe::{
            SimulationEventWorldEntityType, SimulationPipeCharacter, SimulationWorldEvents,
        },
        state::state::TICKS_PER_SECOND,
        types::types::GameOptions,
        weapons::definitions::weapon_def::Weapon,
    };

    use hashlink::{LinkedHashMap, LinkedHashSet};
    use math::math::{
        angle, distance_squared, length, lerp, mix, normalize,
        vector::{ivec2, vec2},
        PI,
    };
    use pool::{
        datatypes::{PoolLinkedHashMap, PoolLinkedHashSet},
        pool::Pool,
        recycle::Recycle,
        traits::Recyclable,
    };
    use serde::{Deserialize, Serialize};

    use super::player::player::Player;

    pub const TICKS_UNTIL_RECOIL_ENDED: GameTickType = 7;

    pub enum DamageTypes<'a> {
        Character(&'a CharacterId),
        CharacterInMatchSide {
            char_id: &'a CharacterId,
            side: MatchSide,
        },
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

        pub emoticon_tick: GameTickCooldownAndLastActionCounter,
        pub cur_emoticon: Option<EmoticonType>,

        pub score: i64,
        pub side: Option<MatchSide>,

        pub eye: TeeEye,
        pub normal_eye_in: GameTickCooldown,

        pub(crate) input: CharacterInput,

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

        pub interactions: LinkedHashSet<CharacterId>,
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
            /// the network stats for this player.
            network_stats: PlayerNetworkStats,
            /// The stage this character is in
            stage_id: StageId,
        },
    }

    #[derive(Debug, Clone, Copy)]
    pub enum FriendlyFireTy {
        Dmg,
        DmgSelf,
        DmgTeam,
        NoDmgTeam,
    }

    #[derive(Debug, Hiarc)]
    pub struct Character {
        pub(crate) base: Entity<CharacterId>,
        pub(crate) core: CharacterCore,
        pub(crate) reusable_core: PoolCharacterReusableCore,
        pub(crate) player_info: PlayerInfo,
        pub(crate) pos: CharacterPos,
        pub(crate) hook: CharacterHook,

        simulation_events: SimulationWorldEvents,
        despawn_info: CharacterDespawnType,

        pub(crate) game_options: GameOptions,

        ty: CharacterPlayerTy,
    }

    impl Character {
        pub fn new(
            id: &CharacterId,
            character_pool: &CharacterPool,
            player_info: PlayerInfo,
            player_input: CharacterInput,
            simulation_events: &SimulationWorldEvents,
            stage_id: &StageId,
            ty: CharacterPlayerTy,
            pos: vec2,
            field: &CharacterPositionPlayfield,
            hooks: &HookedCharacters,
            side: Option<MatchSide>,
            game_options: GameOptions,
        ) -> Self {
            let core = CharacterCore {
                side,
                health: 10,
                armor: 0,
                input: player_input,
                ..Default::default()
            };

            if let CharacterPlayerTy::Player { players, .. } = &ty {
                players.insert(
                    *id,
                    Player {
                        stage_id: *stage_id,
                    },
                );
            }

            simulation_events.push_world(
                Some(*id),
                SimulationEventWorldEntityType::Character {
                    ev: CharacterEvent::Effect {
                        pos: pos / 32.0,
                        ev: GameCharacterEventEffect::Spawn,
                    },
                },
            );
            simulation_events.push_world(
                Some(*id),
                SimulationEventWorldEntityType::Character {
                    ev: CharacterEvent::Sound {
                        pos: Some(pos / 32.0),
                        ev: GameCharacterEventSound::Spawn,
                    },
                },
            );

            let reusable_core = character_pool.character_reusable_cores_pool.new();

            Self {
                base: Entity::new(id),
                core,
                reusable_core,
                player_info,
                pos: field.get_character_pos(pos, *id),
                hook: hooks.get_new_hook(*id),

                simulation_events: simulation_events.clone(),
                despawn_info: Default::default(),

                ty,

                game_options,
            }
        }

        /// Returns `Some` if character is a player's character.
        pub(crate) fn is_player_character(&self) -> Option<PlayerNetworkStats> {
            if let CharacterPlayerTy::Player { network_stats, .. } = &self.ty {
                Some(*network_stats)
            } else {
                None
            }
        }

        pub(crate) fn die(
            &mut self,
            killer_id: Option<CharacterId>,
            weapon: GameWorldActionKillWeapon,
        ) {
            self.despawn_info = CharacterDespawnType::Default(CharacterDespawnInfo {
                pos: *self.pos.pos(),
                respawns_in_ticks: (TICKS_PER_SECOND / 2).into(),
                killer_id,
                weapon,
            });
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
                weapon: GameWorldActionKillWeapon::World,
            });
        }

        /// The character will be dropped and the player will join the spectators
        pub fn despawn_to_join_spectators(&mut self) {
            self.despawn_info = CharacterDespawnType::JoinsSpectator;
        }

        /// normally only useful for snapshot
        pub fn update_player_ty(&mut self, stage_id: &StageId, player_ty: CharacterPlayerTy) {
            match &mut self.ty {
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
                CharacterPlayerTy::Player {
                    players,
                    network_stats,
                    ..
                } => match player_ty {
                    CharacterPlayerTy::None => {
                        players.remove(&self.base.game_element_id);
                        self.ty = player_ty;
                    }
                    CharacterPlayerTy::Player {
                        network_stats: update_stats,
                        ..
                    } => {
                        *network_stats = update_stats;
                    }
                },
            }
        }

        pub fn give_ninja(&mut self) {
            let buff = self
                .reusable_core
                .buffs
                .entry(CharacterBuff::Ninja)
                .or_insert_with(|| BuffProps {
                    remaining_tick: 0.into(),
                    interact_tick: 0.into(),
                    interact_cursor_dir: vec2::default(),
                    interact_val: 0.0,
                });
            buff.remaining_tick = (15 * TICKS_PER_SECOND).into();
            self.core.normal_eye_in = TICKS_PER_SECOND.into();
            self.core.eye = TeeEye::Angry;
            self.core
                .attack_recoil
                .advance_ticks_passed_to_cooldown_len();
        }

        fn push_ev(&self, ev: CharacterEvent) {
            self.simulation_events.push_world(
                Some(self.base.game_element_id),
                SimulationEventWorldEntityType::Character { ev },
            );
        }

        #[must_use]
        fn handle_tiles(&mut self, old_pos: vec2, collision: &Collision) -> CharacterDamageResult {
            let mut res = CharacterDamageResult::None;
            let cur_pos = *self.pos.pos();
            collision.intersect_line_feedback(&old_pos, &cur_pos, |tile| match tile {
                HitTile::Game(tile) => {
                    if tile.index == DdraceTileNum::Death as u8 {
                        self.die(None, GameWorldActionKillWeapon::World);
                        res = CharacterDamageResult::Death;
                    }
                }
                HitTile::Front(tile) => {
                    if tile.index == DdraceTileNum::Death as u8 {
                        self.die(None, GameWorldActionKillWeapon::World);
                        res = CharacterDamageResult::Death;
                    }
                }
                HitTile::Tele(_) => {}
                HitTile::Speedup(_) => {}
                HitTile::Switch(_) => {}
                HitTile::Tune(_) => {
                    // tune tiles are handled on the fly where needed
                }
            });
            res
        }

        fn set_weapon(&mut self, new_weapon: WeaponType) {
            if self.core.active_weapon == new_weapon {
                return;
            }

            self.core.prev_weapon = self.core.active_weapon;
            self.core.queued_weapon = None;
            self.core.active_weapon = new_weapon;
            self.push_ev(CharacterEvent::Sound {
                pos: Some(*self.pos.pos() / 32.0),
                ev: GameCharacterEventSound::WeaponSwitch { new_weapon },
            });

            if self.core.active_weapon as usize >= WeaponType::COUNT {
                self.core.active_weapon = Default::default(); // TODO: what is the idea behind this?
            }
            if self
                .reusable_core
                .weapons
                .get_mut(&self.core.active_weapon)
                .is_some()
            {
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

        pub fn friendly_fire_no_dmg(
            characters: &dyn CharactersGetter,
            self_char_id: &CharacterId,
            attacker_char_id: &CharacterId,
            attacker_fallback_side: Option<MatchSide>,
        ) -> FriendlyFireTy {
            if self_char_id.eq(attacker_char_id) {
                return FriendlyFireTy::DmgSelf;
            }
            let self_side = characters.side(self_char_id);
            let other_side = characters.side(attacker_char_id).or(attacker_fallback_side);
            let Some((self_side, other_side)) = self_side.zip(other_side) else {
                return FriendlyFireTy::Dmg;
            };

            if characters
                .does_friendly_fire(self_char_id)
                .or_else(|| characters.does_friendly_fire(attacker_char_id))
                .unwrap_or_default()
            {
                return FriendlyFireTy::DmgTeam;
            }

            if self_side == other_side {
                FriendlyFireTy::NoDmgTeam
            } else {
                FriendlyFireTy::Dmg
            }
        }

        fn create_damage_indicators(&self, pos: &vec2, angle: f32, amount: usize) {
            let a = 3.0 * PI / 2.0 + angle;
            let s = a - PI / 3.0;
            let e = a + PI / 3.0;
            for i in 0..amount {
                let f = mix(&s, &e, (i + 1) as f32 / (amount + 2) as f32);

                let angle = f;
                let dir = vec2::new(angle.cos(), angle.sin()) * -75.0 / 4.0;
                self.push_ev(CharacterEvent::Effect {
                    pos: *pos / 32.0,
                    ev: GameCharacterEventEffect::DamageIndicator { vel: dir },
                });
            }
        }

        fn create_hammer_hit(&self, pos: &vec2) {
            self.push_ev(CharacterEvent::Effect {
                pos: *pos / 32.0,
                ev: GameCharacterEventEffect::HammerHit,
            });
            self.push_ev(CharacterEvent::Sound {
                pos: Some(*pos / 32.0),
                ev: GameCharacterEventSound::HammerHit,
            });
        }

        pub fn take_damage_from(
            self_char: &mut Character,
            self_char_id: &CharacterId,
            killer_id: CharacterId,
            force: &vec2,
            _source: &vec2,
            mut dmg_amount: u32,
            from: DamageTypes,
            by: DamageBy,
        ) -> CharacterDamageResult {
            let core = &mut self_char.core;
            core.core.vel += *force;
            let old_health = core.health;
            let old_armor = core.armor;
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

                let indicator_amount =
                    ((old_health - core.health) + (old_armor - core.armor)) as usize;
                self_char.create_damage_indicators(self_char.pos.pos(), 0.0, indicator_amount);
                let id = match from {
                    DamageTypes::Character(id) => id,
                    DamageTypes::CharacterInMatchSide { char_id, .. } => char_id,
                };

                if *id != *self_char_id {
                    self_char.push_ev(CharacterEvent::Sound {
                        pos: Some(*self_char.pos.pos() / 32.0),
                        ev: GameCharacterEventSound::Hit { strong: false },
                    });
                }

                let core = &mut self_char.core;
                // check for death
                if core.health == 0 {
                    self_char.die(
                        Some(killer_id),
                        match by {
                            DamageBy::Ninja => GameWorldActionKillWeapon::Ninja,
                            DamageBy::Weapon(weapon) => {
                                GameWorldActionKillWeapon::Weapon { weapon }
                            }
                        },
                    );

                    return CharacterDamageResult::Death;
                }

                self_char.push_ev(CharacterEvent::Sound {
                    pos: Some(*self_char.pos.pos() / 32.0),
                    ev: GameCharacterEventSound::Pain {
                        long: dmg_amount > 2,
                    },
                });

                let core = &mut self_char.core;
                core.normal_eye_in = (TICKS_PER_SECOND / 2).into();
                core.eye = TeeEye::Pain;

                CharacterDamageResult::Damage
            } else {
                CharacterDamageResult::None
            }
        }

        pub fn take_damage(
            characters: &mut dyn CharactersGetter,
            self_char_id: &CharacterId,
            force: &vec2,
            source: &vec2,
            mut dmg_amount: u32,
            from: DamageTypes,
            by: DamageBy,
        ) -> CharacterDamageResult {
            let (killer_id, friendly_fire_ty) = match &from {
                DamageTypes::Character(&from_id) => {
                    let friendly_fire_ty =
                        Self::friendly_fire_no_dmg(characters, self_char_id, &from_id, None);
                    (from_id, friendly_fire_ty)
                }
                DamageTypes::CharacterInMatchSide {
                    char_id: &char_id,
                    side,
                } => {
                    let friendly_fire_ty =
                        Self::friendly_fire_no_dmg(characters, self_char_id, &char_id, Some(*side));
                    (char_id, friendly_fire_ty)
                }
            };
            match friendly_fire_ty {
                FriendlyFireTy::Dmg => {
                    // ignore
                }
                FriendlyFireTy::DmgSelf | FriendlyFireTy::DmgTeam => {
                    dmg_amount = 1.max(dmg_amount / 2);
                }
                FriendlyFireTy::NoDmgTeam => {
                    dmg_amount = 0;
                }
            }

            let self_char = characters.char_mut(self_char_id).unwrap();
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
                (&res, characters.char_mut(&killer_id))
            {
                if let FriendlyFireTy::Dmg = friendly_fire_ty {
                    killer.core.eye = TeeEye::Happy;
                    killer.core.normal_eye_in = (TICKS_PER_SECOND / 2).into();
                }
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

            let full_auto = self.core.active_weapon == WeaponType::Grenade
                || self.core.active_weapon == WeaponType::Shotgun
                || self.core.active_weapon == WeaponType::Laser;

            let auto_fired = full_auto && *self.core.input.state.fire;
            let fired = fire.is_some();

            let direction = normalize(&{
                let cursor_pos = if fired {
                    fire.as_ref().unwrap().1.to_vec2()
                } else {
                    self.core.input.cursor.to_vec2()
                };
                vec2::new(cursor_pos.x as f32, cursor_pos.y as f32)
            });

            // check if we gonna fire
            let will_fire = fired || auto_fired;

            if !will_fire {
                return;
            }

            // check for ammo
            let cur_weapon = self.reusable_core.weapons.get_mut(&self.core.active_weapon);
            if cur_weapon
                .as_ref()
                .is_none_or(|weapon| weapon.cur_ammo.is_some_and(|val| val == 0))
            {
                if fired && self.core.no_ammo_sound.is_none() {
                    self.push_ev(CharacterEvent::Sound {
                        pos: Some(*self.pos.pos() / 32.0),
                        ev: GameCharacterEventSound::NoAmmo {
                            weapon: self.core.active_weapon,
                        },
                    });
                    self.core.no_ammo_sound = TICKS_PER_SECOND.into();
                }
                return;
            }

            let proj_start_pos = *self.pos.pos() + direction * PHYSICAL_SIZE * 0.75;

            // TODO: check all branches. make sure no code/TODO comments are in, before removing this comment

            self.core.attack_recoil = match self.core.active_weapon {
                WeaponType::Hammer => {
                    // TODO: recheck
                    self.push_ev(CharacterEvent::Sound {
                        pos: Some(*self.pos.pos() / 32.0),
                        ev: GameCharacterEventSound::HammerFire,
                    });

                    let mut hits = 0;
                    let core_pos = *self.pos.pos();
                    pipe.characters.for_other_characters_in_range_mut(
                        &proj_start_pos,
                        PHYSICAL_SIZE * 0.5,
                        &mut |char, removed_characters| {
                            if pipe.collision.intersect_line(
                                &proj_start_pos,
                                char.pos.pos(),
                                &mut vec2::default(),
                                &mut vec2::default(),
                                CollisionTypes::SOLID,
                            ) != CollisionTile::None
                            {
                                return;
                            }

                            // set his velocity to fast upward (for now)
                            if length(&(*char.pos.pos() - proj_start_pos)) > 0.0 {
                                self.create_hammer_hit(
                                    &(*char.pos.pos()
                                        - normalize(&(*char.pos.pos() - proj_start_pos))
                                            * PHYSICAL_SIZE
                                            * 0.5),
                                );
                            } else {
                                self.create_hammer_hit(&proj_start_pos);
                            }

                            let dir = if length(&(*char.pos.pos() - core_pos)) > 0.0 {
                                normalize(&(*char.pos.pos() - core_pos))
                            } else {
                                vec2::new(0.0, -1.0)
                            };

                            let char_id = char.base.game_element_id;
                            let self_id = self.base.game_element_id;
                            if Self::take_damage(
                                &mut (
                                    (self.base.game_element_id, &mut *self),
                                    (char_id, &mut *char),
                                ),
                                &char_id,
                                &(vec2::new(0.0, -1.0)
                                    + normalize(&(dir + vec2::new(0.0, -1.1))) * 10.0),
                                &(dir * -1.0),
                                3,
                                DamageTypes::Character(&self_id),
                                DamageBy::Weapon(WeaponType::Hammer),
                            ) == CharacterDamageResult::Death
                            {
                                removed_characters.insert(char.base.game_element_id);
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
                    pipe.entity_events.push(CharacterTickEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                        ty: WeaponType::Gun,
                        lifetime: tunings.gun_lifetime,
                    });
                    self.push_ev(CharacterEvent::Sound {
                        pos: Some(*self.pos.pos() / 32.0),
                        ev: GameCharacterEventSound::GunFire,
                    });

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

                        pipe.entity_events.push(CharacterTickEvent::Projectile {
                            pos: proj_start_pos,
                            dir: vec2::new(a.cos(), a.sin()) * speed,
                            ty: WeaponType::Shotgun,
                            lifetime: tunings.shotgun_lifetime,
                        });
                    }

                    self.push_ev(CharacterEvent::Sound {
                        pos: Some(*self.pos.pos() / 32.0),
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
                    pipe.entity_events.push(CharacterTickEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                        ty: WeaponType::Grenade,
                        lifetime: tunings.grenade_lifetime,
                    });
                    self.push_ev(CharacterEvent::Sound {
                        pos: Some(*self.pos.pos() / 32.0),
                        ev: GameCharacterEventSound::GrenadeFire,
                    });
                    let fire_delay = tunings.grenade_fire_delay;
                    ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into()
                }
                WeaponType::Laser => {
                    pipe.entity_events.push(CharacterTickEvent::Laser {
                        pos: *self.pos.pos(),
                        dir: direction,
                        energy: pipe.collision.get_tune_at(self.pos.pos()).laser_reach,
                        can_hit_own: self.game_options.laser_hit_self,
                    });
                    self.push_ev(CharacterEvent::Sound {
                        pos: Some(*self.pos.pos() / 32.0),
                        ev: GameCharacterEventSound::LaserFire,
                    });

                    let fire_delay = pipe.collision.get_tune_at(&proj_start_pos).laser_fire_delay;
                    ((fire_delay * TICKS_PER_SECOND as f32 / 1000.0).ceil() as GameTickType).into()
                }
            };

            let cur_weapon = self
                .reusable_core
                .weapons
                .get_mut(&self.core.active_weapon)
                .unwrap();
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

            self.simulation_events.push_world(
                Some(self.base.game_element_id),
                SimulationEventWorldEntityType::Character {
                    ev: CharacterEvent::BuffSound {
                        pos: Some(*self.pos.pos() / 32.0),
                        ev: GameBuffSoundEvent::Ninja(GameBuffNinjaEventSound::Attack),
                    },
                },
            );
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
                    &ivec2::new(PHYSICAL_SIZE as i32, PHYSICAL_SIZE as i32),
                    0.0,
                );
                self.pos.move_pos(new_pos);

                self.core.core.vel = vec2::new(0.0, 0.0);

                let dir = *self.pos.pos() - old_pos;
                let center = old_pos + dir * 0.5;
                pipe.characters.for_other_characters_in_range_mut(
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

                        let self_id = self.base.game_element_id;
                        let self_pos = *self.pos.pos();
                        if Self::take_damage(
                            &mut (
                                (self.base.game_element_id, &mut *self),
                                (char_id, &mut *char),
                            ),
                            &char_id,
                            &vec2::new(0.0, -10.0),
                            &self_pos,
                            9,
                            DamageTypes::Character(&self_id),
                            DamageBy::Ninja,
                        ) == CharacterDamageResult::Death
                        {
                            removed_chars.insert(char_id);
                        }

                        self.simulation_events.push_world(
                            Some(self.base.game_element_id),
                            SimulationEventWorldEntityType::Character {
                                ev: CharacterEvent::BuffSound {
                                    pos: Some(*self.pos.pos() / 32.0),
                                    ev: GameBuffSoundEvent::Ninja(GameBuffNinjaEventSound::Hit),
                                },
                            },
                        );
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
            self.core.emoticon_tick.tick();
        }
    }

    impl EntityInterface<CharacterCore, CharacterReusableCore, SimulationPipeCharacter<'_>>
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

            let old_pos = *self.pos.pos();
            let mut core_pipe = CorePipe {
                characters: pipe.characters,
                input: &self.core.input,
            };
            self.core.core.physics_tick(
                &mut self.pos,
                &mut self.hook,
                true,
                true,
                &mut core_pipe,
                pipe.collision,
                CoreEvents {
                    character_id: &self.base.game_element_id,
                    simulation_events: &self.simulation_events,
                },
            );

            if Entity::<CharacterId>::outside_of_playfield(self.pos.pos(), pipe.collision) {
                self.die(None, GameWorldActionKillWeapon::World);
                return EntityTickResult::RemoveEntity;
            }

            let tiles_res = self.handle_tiles(old_pos, pipe.collision);
            if matches!(tiles_res, CharacterDamageResult::Death) {
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
            };
            self.core
                .core
                .physics_move(&mut self.pos, &mut core_pipe, pipe.collision);
            self.core
                .core
                .physics_quantize(&mut self.pos, &mut self.hook);

            EntityTickResult::None
        }

        fn drop_mode(&mut self, mode: DropMode) {
            self.base.drop_mode = mode;
        }
    }

    impl Drop for Character {
        fn drop(&mut self) {
            let (is_dead, add_to_no_char_players, death_effect) = match &mut self.despawn_info {
                CharacterDespawnType::Default(despawn_info) => {
                    let is_dead = despawn_info.respawns_in_ticks.is_some();
                    if matches!(self.base.drop_mode, DropMode::None) {
                        self.simulation_events.push_world(
                            Some(self.base.game_element_id),
                            SimulationEventWorldEntityType::Character {
                                ev: CharacterEvent::Despawn {
                                    killer_id: despawn_info.killer_id,
                                    weapon: despawn_info.weapon,
                                },
                            },
                        );
                    }
                    (is_dead, true, true)
                }
                CharacterDespawnType::DropFromGame => (false, false, true),
                CharacterDespawnType::JoinsSpectator => (false, true, false),
            };

            let (is_dead, add_to_no_char_players, death_effect) = (
                is_dead && matches!(self.base.drop_mode, DropMode::None | DropMode::NoEvents),
                add_to_no_char_players
                    && matches!(self.base.drop_mode, DropMode::None | DropMode::NoEvents),
                death_effect && matches!(self.base.drop_mode, DropMode::None),
            );

            if death_effect {
                self.simulation_events.push_world(
                    Some(self.base.game_element_id),
                    SimulationEventWorldEntityType::Character {
                        ev: CharacterEvent::Effect {
                            pos: *self.pos.pos() / 32.0,
                            ev: GameCharacterEventEffect::Death,
                        },
                    },
                );
                self.simulation_events.push_world(
                    Some(self.base.game_element_id),
                    SimulationEventWorldEntityType::Character {
                        ev: CharacterEvent::Sound {
                            pos: Some(*self.pos.pos() / 32.0),
                            ev: GameCharacterEventSound::Death,
                        },
                    },
                );
            }

            if let CharacterPlayerTy::Player {
                players,
                no_char_players,
                network_stats,
                stage_id,
            } = &self.ty
            {
                players.remove(&self.base.game_element_id);
                if add_to_no_char_players {
                    no_char_players.insert(
                        self.base.game_element_id,
                        NoCharPlayer::new(
                            self.player_info.clone(),
                            self.core.input,
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
                                    side: self.core.side,
                                    score: self.core.score,
                                    stage_id: *stage_id,
                                    died_at_pos: *self.pos.pos(),
                                }
                            } else {
                                NoCharPlayerType::Spectator {
                                    spectated_character: None,
                                }
                            },
                            *network_stats,
                        ),
                    );
                }
            }
        }
    }

    pub type PoolCharacters = LinkedHashMap<CharacterId, Character>;

    pub type CharactersView<'a, F> = LinkedHashMapView<'a, CharacterId, Character, F>;

    pub type Characters = PoolLinkedHashMap<CharacterId, Character>;

    pub trait CharactersGetter {
        fn char_mut(&mut self, char_id: &CharacterId) -> Option<&mut Character>;
        fn side(&self, char_id: &CharacterId) -> Option<MatchSide>;
        fn does_friendly_fire(&self, char_id: &CharacterId) -> Option<bool>;
    }

    impl CharactersGetter for Characters {
        fn char_mut(&mut self, char_id: &CharacterId) -> Option<&mut Character> {
            self.get_mut(char_id)
        }
        fn side(&self, char_id: &CharacterId) -> Option<MatchSide> {
            self.get(char_id).and_then(|c| c.core.side)
        }
        fn does_friendly_fire(&self, char_id: &CharacterId) -> Option<bool> {
            self.get(char_id).map(|c| c.game_options.friendly_fire)
        }
    }

    impl CharactersGetter for ((CharacterId, &mut Character), (CharacterId, &mut Character)) {
        fn char_mut(&mut self, char_id: &CharacterId) -> Option<&mut Character> {
            if self.0 .0 == *char_id {
                Some(self.0 .1)
            } else {
                Some(self.1 .1)
            }
        }
        fn side(&self, char_id: &CharacterId) -> Option<MatchSide> {
            if self.0 .0 == *char_id {
                &*self.0 .1
            } else {
                &*self.1 .1
            }
            .core
            .side
        }
        fn does_friendly_fire(&self, char_id: &CharacterId) -> Option<bool> {
            Some(
                if self.0 .0 == *char_id {
                    &*self.0 .1
                } else {
                    &*self.1 .1
                }
                .game_options
                .friendly_fire,
            )
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct RemovedCharacters {
        ids: PoolLinkedHashSet<CharacterId>,
        pool: Pool<LinkedHashSet<CharacterId>>,
    }

    #[hiarc_safer_rc_refcell]
    impl Default for RemovedCharacters {
        fn default() -> Self {
            let pool = Pool::with_capacity(2);
            let ids = pool.new();
            Self { ids, pool }
        }
    }

    #[hiarc_safer_rc_refcell]
    impl RemovedCharacters {
        pub fn insert(&mut self, id: CharacterId) {
            self.ids.insert(id);
        }
        pub fn is_empty(&self) -> bool {
            self.ids.is_empty()
        }
        pub fn contains(&self, id: &CharacterId) -> bool {
            self.ids.contains(id)
        }
        pub fn clear(&mut self) {
            self.ids.clear();
        }
        pub fn take(&mut self) -> PoolLinkedHashSet<CharacterId> {
            std::mem::replace(&mut self.ids, self.pool.new())
        }
    }

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
