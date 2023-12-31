pub mod character_core {
    use std::{collections::HashSet, ops::AddAssign};

    use num_derive::FromPrimitive;
    use shared_base::{
        game_types::TGameElementID,
        mapdef::TileNum,
        network::messages::MsgObjPlayerInput,
        reuseable::{CloneWithCopyableElements, ReusableCore},
    };

    use crate::{
        collision::collision::Collision, player::player::PlayerInput,
        simulation_pipe::simulation_pipe::SimulationPipeCharactersGetter,
        state::state::TICKS_PER_SECOND,
    };

    use math::math::{
        closest_point_on_line, distance, dot, length, mix, normalize, round_to_int, vector::vec2,
        PI,
    };

    use num_traits::FromPrimitive;

    use bincode::{BorrowDecode, Decode, Encode};
    use pool::traits::Recyclable;
    use serde::{Deserialize, Serialize};

    // hooking stuff
    #[derive(
        Debug,
        Default,
        Clone,
        Copy,
        Serialize,
        Deserialize,
        Encode,
        Decode,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        FromPrimitive,
    )]
    pub enum HookState {
        Retracted = 0,
        #[default]
        Idle,
        RetractStart,
        RetractEnd,
        HookFlying,
        HookGrabbed,
    }

    enum CoreEvent {
        GroundJump = 0x01,
        AirJump = 0x02,
        HookLaunch = 0x04,
        HookAttachPlayer = 0x08,
        HookAttachGround = 0x10,
        HookHitNoHook = 0x20,
        HookRetract = 0x40,
        // COREEVENT_HOOK_TELE=0x80,
    }

    #[derive(Serialize, Deserialize, Default, Encode, Decode)]
    pub struct NetObjCharacterCore {
        pub tick: i32,
        pub x: i32,
        pub y: i32,
        pub vel_x: i32,
        pub vel_y: i32,
        pub angle: i32,
        pub direction: i32,
        pub jumped: i32,
        pub hooked_player: Option<TGameElementID>,
        pub hook_state: i32,
        pub hook_tick: i32,
        pub hook_x: i32,
        pub hook_y: i32,
        pub hook_dx: i32,
        pub hook_dy: i32,
    }

    /*#include "gamecore.h"

    #include "collision.h"
    #include "mapitems.h"
    #include "teamscore.h"

    #include <engine/shared/config.h>

    const char *CTuningParams::ms_apNames[] =
    {
    #define MACRO_TUNING_PARAM(Name, ScriptName, Value, Description) #ScriptName,
    #include "tuning.h"
    #undef MACRO_TUNING_PARAM
    };

    bool CTuningParams::Set(int Index, float Value)
    {
    if(Index < 0 || Index >= Num())
        return false;
    ((CTuneParam *)this)[Index] = Value;
    return true;
    }

    bool CTuningParams::Get(int Index, float *pValue) const
    {
    if(Index < 0 || Index >= Num())
        return false;
    *pValue = (float)((CTuneParam *)this)[Index];
    return true;
    }

    bool CTuningParams::Set(const char *pName, float Value)
    {
    for(int i = 0; i < Num(); i++)
        if(str_comp_nocase(pName, Name(i)) == 0)
            return Set(i, Value);
    return false;
    }

    bool CTuningParams::Get(const char *pName, float *pValue) const
    {
    for(int i = 0; i < Num(); i++)
        if(str_comp_nocase(pName, Name(i)) == 0)
            return Get(i, pValue);

    return false;
    }

    int CTuningParams::PossibleTunings(const char *pStr, IConsole::FPossibleCallback pfnCallback, void *pUser)
    {
    int Index = 0;
    for(int i = 0; i < Num(); i++)
    {
        if(str_find_nocase(Name(i), pStr))
        {
            pfnCallback(Index, Name(i), pUser);
            Index++;
        }
    }
    return Index;
    }

    float CTuningParams::GetWeaponFireDelay(int Weapon) const
    {
    switch(Weapon)
    {
    case WEAPON_HAMMER: return (float)core.m_HammerHitFireDelay / 1000.0f;
    case WEAPON_GUN: return (float)core.m_GunFireDelay / 1000.0f;
    case WEAPON_SHOTGUN: return (float)core.m_ShotgunFireDelay / 1000.0f;
    case WEAPON_GRENADE: return (float)core.m_GrenadeFireDelay / 1000.0f;
    case WEAPON_LASER: return (float)core.m_LaserFireDelay / 1000.0f;
    case WEAPON_NINJA: return (float)core.m_NinjaFireDelay / 1000.0f;
    default: dbg_assert(false, "invalid weapon"); return 0.0f; // this value should not be reached
    }
    }

    float VelocityRamp(float Value, float Start, float Range, float Curvature)
    {
    if(Value < Start)
        return 1.0f;
    return 1.0f / powf(Curvature, (Value - Start) / Range);
    }

    void CCharacterCore::Init(CWorldCore *pWorld, CCollision *pCollision, CTeamsCore *pTeams, std::map<int, std::vector<vec2>> *pTeleOuts)
    {
    core.m_pWorld = pWorld;
    core.m_pCollision = pCollision;
    core.m_pTeleOuts = pTeleOuts;

    core.m_pTeams = pTeams;
    core.m_Id = -1;

    // fail safe, if core's tuning didn't get updated at all, just fallback to world tuning.
    core.m_Tuning = core.m_pWorld->core.m_aTuning[g_Config.m_ClDummy];
    Reset();
    }

    void CCharacterCore::Reset()
    {
    core.m_Pos = vec2(0, 0);
    core.m_Vel = vec2(0, 0);
    core.m_NewHook = false;
    core.m_HookPos = vec2(0, 0);
    core.m_HookDir = vec2(0, 0);
    core.m_HookTick = 0;
    core.m_HookState = HOOK_IDLE;
    SetHookedPlayer(-1);
    core.m_AttachedPlayers.clear();
    core.m_Jumped = 0;
    core.m_JumpedTotal = 0;
    core.m_Jumps = 2;
    core.m_TriggeredEvents = 0;

    // DDNet Character
    core.m_Solo = false;
    core.m_Jetpack = false;
    core.m_CollisionDisabled = false;
    core.m_EndlessHook = false;
    core.m_EndlessJump = false;
    core.m_HammerHitDisabled = false;
    core.m_GrenadeHitDisabled = false;
    core.m_LaserHitDisabled = false;
    core.m_ShotgunHitDisabled = false;
    core.m_HookHitDisabled = false;
    core.m_Super = false;
    core.m_HasTelegunGun = false;
    core.m_HasTelegunGrenade = false;
    core.m_HasTelegunLaser = false;
    core.m_FreezeStart = 0;
    core.m_FreezeEnd = 0;
    core.m_IsInFreeze = false;
    core.m_DeepFrozen = false;
    core.m_LiveFrozen = false;

    // never initialize both to 0
    core.m_Input.m_TargetX = 0;
    core.m_Input.m_TargetY = -1;
    }
    */
    #[derive(Debug, Copy, Clone, Encode, Decode)]
    pub struct Tunings {
        pub ground_control_speed: f32,
        pub ground_control_accel: f32,
        pub ground_friction: f32,
        pub ground_jump_impulse: f32,
        pub air_jump_impulse: f32,
        pub air_control_speed: f32,
        pub air_control_accel: f32,
        pub air_friction: f32,
        pub hook_length: f32,
        pub hook_fire_speed: f32,
        pub hook_drag_accel: f32,
        pub hook_drag_speed: f32,
        pub gravity: f32,
        pub velramp_start: f32,
        pub velramp_range: f32,
        pub velramp_curvature: f32,
        pub gun_curvature: f32,
        pub gun_speed: f32,
        pub gun_lifetime: f32,
        pub shotgun_curvature: f32,
        pub shotgun_speed: f32,
        pub shotgun_speeddiff: f32,
        pub shotgun_lifetime: f32,
        pub grenade_curvature: f32,
        pub grenade_speed: f32,
        pub grenade_lifetime: f32,
        pub laser_reach: f32,
        pub laser_bounce_delay: f32,
        pub laser_bounce_num: f32,
        pub laser_bounce_cost: f32,
        pub laser_damage: f32,
        pub player_collision: f32,
        pub player_hooking: f32,
        pub jetpack_strength: f32,
        pub shotgun_strength: f32,
        pub explosion_strength: f32,
        pub hammer_strength: f32,
        pub hook_duration: f32,
        pub hammer_fire_delay: f32,
        pub gun_fire_delay: f32,
        pub shotgun_fire_delay: f32,
        pub grenade_fire_delay: f32,
        pub laser_fire_delay: f32,
        pub ninja_fire_delay: f32,
        pub hammer_hit_fire_delay: f32,
    }

    impl Default for Tunings {
        fn default() -> Self {
            Self {
                ground_control_speed: 10.0,
                ground_control_accel: 100.0 / TICKS_PER_SECOND as f32,
                ground_friction: 0.5,
                ground_jump_impulse: 13.2,
                air_jump_impulse: 12.0,
                air_control_speed: 250.0 / TICKS_PER_SECOND as f32,
                air_control_accel: 1.5,
                air_friction: 0.95,
                hook_length: 380.0,
                hook_fire_speed: 80.0,
                hook_drag_accel: 3.0,
                hook_drag_speed: 15.0,
                gravity: 0.5,
                velramp_start: 550.0,
                velramp_range: 2000.0,
                velramp_curvature: 1.4,
                gun_curvature: 1.25,
                gun_speed: 2200.0,
                gun_lifetime: 2.0,
                shotgun_curvature: 1.25,
                shotgun_speed: 2750.0,
                shotgun_speeddiff: 0.8,
                shotgun_lifetime: 0.20,
                grenade_curvature: 7.0,
                grenade_speed: 1000.0,
                grenade_lifetime: 2.0,
                laser_reach: 800.0,
                laser_bounce_delay: 150.0,
                laser_bounce_num: 1000.0,
                laser_bounce_cost: 0.0,
                laser_damage: 5.0,
                player_collision: 1.0,
                player_hooking: 1.0,
                jetpack_strength: 400.0,
                shotgun_strength: 10.0,
                explosion_strength: 6.0,
                hammer_strength: 1.0,
                hook_duration: 1.25,
                hammer_fire_delay: 125.0,
                gun_fire_delay: 125.0,
                shotgun_fire_delay: 500.0,
                grenade_fire_delay: 500.0,
                laser_fire_delay: 800.0,
                ninja_fire_delay: 800.0,
                hammer_hit_fire_delay: 320.0,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
    pub struct HookedCharacter {
        pub(crate) id: Option<TGameElementID>,
        pub(crate) attached_characters_ids: HashSet<TGameElementID>,
    }

    impl HookedCharacter {
        pub fn new() -> Self {
            Self {
                id: Default::default(),
                attached_characters_ids: Default::default(),
            }
        }

        pub fn get_hooked_char(&self) -> &Option<TGameElementID> {
            &self.id
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
    pub struct CoreReusable {
        pub hooked_character: HookedCharacter,
    }

    impl CloneWithCopyableElements for CoreReusable {
        fn copy_clone_from(&mut self, other: &Self) {
            self.hooked_character
                .id
                .clone_from(&other.hooked_character.id);
            self.hooked_character
                .attached_characters_ids
                .copy_clone_from(&other.hooked_character.attached_characters_ids);
        }
    }

    impl Recyclable for CoreReusable {
        fn new() -> Self {
            Self {
                hooked_character: HookedCharacter::new(),
            }
        }

        fn reset(&mut self) {
            self.hooked_character.attached_characters_ids.reset()
        }
    }

    impl ReusableCore for CoreReusable {}

    #[derive(Debug, Copy, Clone, Default)]
    pub struct Core {
        pub pos: vec2,
        pub vel: vec2,

        pub hook_pos: vec2,
        hook_dir: vec2,
        hook_tele_base: vec2,
        hook_tick: i32,
        pub hook_state: HookState,
        _active_weapon: i32,
        /*struct WeaponStat
            {
                int m_AmmoRegenStart,
                int m_Ammo,
                int m_Ammocost,
                bool m_Got,
            } m_aWeapons[NUM_WEAPONS],*/

            // ninja
            /*struct
            {
                vec2 m_ActivationDir,
                int m_ActivationTick,
                int m_CurrentMoveTime,
                int m_OldVelAmount,
            } m_Ninja,
        */
        new_hook: bool,

        jumped: i32,
        // m_JumpedTotal counts the jumps performed in the air
        jumped_total: i32,
        jumps: i32,

        direction: i32,
        angle: i32,
        //TODO CNetObj_PlayerInput m_Input,
        triggered_events: i32,

        // DDRace
        reset: bool,

        _last_vel: vec2,
        colliding: i32,
        left_wall: bool,

        // DDNet Character
        solo: bool,
        _jetpack: bool,
        collision_disabled: bool,
        _endless_hook: bool,
        _endless_jump: bool,
        _hammer_hit_disabled: bool,
        _grenade_hit_disabled: bool,
        _laser_hit_disabled: bool,
        _shotgun_hit_disabled: bool,
        hook_hit_disabled: bool,
        is_super: bool,
        _has_telegun_gun: bool,
        _has_telegun_grenade: bool,
        _has_telegun_laser: bool,
        _freeze_start: i32,
        _freeze_end: i32,
        _is_in_freeze: bool,
        _deep_frozen: bool,
        _live_frozen: bool,

        //CTeamsCore *m_pTeams,
        move_restrictions: i32,

        tuning: Tunings,
    }

    impl Core {
        pub fn physics_write(&self, net_core: &mut NetObjCharacterCore) {
            net_core.x = round_to_int(self.pos.x);
            net_core.y = round_to_int(self.pos.y);

            net_core.vel_x = round_to_int(self.vel.x * 256.0);
            net_core.vel_y = round_to_int(self.vel.y * 256.0);
            net_core.hook_state = self.hook_state as i32;
            net_core.hook_tick = self.hook_tick;
            net_core.hook_x = round_to_int(self.hook_pos.x);
            net_core.hook_y = round_to_int(self.hook_pos.y);
            net_core.hook_dx = round_to_int(self.hook_dir.x * 256.0);
            net_core.hook_dy = round_to_int(self.hook_dir.y * 256.0);
            net_core.jumped = self.jumped;
            net_core.direction = self.direction;
            net_core.angle = self.angle;
        }

        fn physics_read(&mut self, net_core: &NetObjCharacterCore) {
            self.pos.x = net_core.x as f32;
            self.pos.y = net_core.y as f32;
            self.vel.x = net_core.vel_x as f32 / 256.0;
            self.vel.y = net_core.vel_y as f32 / 256.0;
            self.hook_state = HookState::from_i32(net_core.hook_state).unwrap();
            self.hook_tick = net_core.hook_tick;
            self.hook_pos.x = net_core.hook_x as f32;
            self.hook_pos.y = net_core.hook_y as f32;
            self.hook_dir.x = net_core.hook_dx as f32 / 256.0;
            self.hook_dir.y = net_core.hook_dy as f32 / 256.0;
            self.jumped = net_core.jumped;
            self.direction = net_core.direction;
            self.angle = net_core.angle;
        }
    }

    impl Encode for Core {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            let mut net_core = NetObjCharacterCore::default();
            self.physics_write(&mut net_core);
            let conf = *encoder.config();
            bincode::encode_into_writer(&net_core, encoder.writer(), conf)?;
            Ok(())
        }
    }

    impl Decode for Core {
        fn decode<D: bincode::de::Decoder>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            let conf = *decoder.config();
            let res_decode =
                bincode::decode_from_reader::<NetObjCharacterCore, _, _>(decoder.reader(), conf)?;
            let mut res = Self::default();
            res.physics_read(&res_decode);
            Ok(res)
        }
    }

    impl<'de> BorrowDecode<'de> for Core {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Self::decode(decoder)
        }
    }

    impl Serialize for Core {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut net_core = NetObjCharacterCore::default();
            self.physics_write(&mut net_core);
            Ok(net_core.serialize(serializer).unwrap())
        }
    }

    impl<'de> Deserialize<'de> for Core {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let res_decode = NetObjCharacterCore::deserialize(deserializer)?;
            let mut res = Self::default();
            res.physics_read(&res_decode);
            Ok(res)
        }
    }

    pub struct CorePipe<'a> {
        pub(crate) characters: &'a mut dyn SimulationPipeCharactersGetter,
        pub(crate) input: &'a PlayerInput,
        pub(crate) reusable_core: &'a mut CoreReusable,
        pub(crate) character_id: &'a TGameElementID,
    }

    impl<'a> CorePipe<'a> {
        fn get_other_character_id_and_cores_iter(
            &self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &Core),
        ) {
            self.characters
                .get_other_character_id_and_cores_iter(for_each_func)
        }

        fn get_other_character_id_and_cores_iter_mut(
            &mut self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &mut Core, &mut CoreReusable),
        ) {
            self.characters
                .get_other_character_id_and_cores_iter_mut(for_each_func)
        }

        fn get_other_character_core_by_id(&self, other_char_id: &TGameElementID) -> &Core {
            self.characters
                .get_other_character_core_by_id(other_char_id)
        }

        fn set_or_reset_hooked_char(&mut self, id: Option<TGameElementID>) {
            let cur_id = self.reusable_core.hooked_character.id;
            self.reusable_core.hooked_character.id = id.clone();
            // if the player was attached to smth, deattach it
            if let Some(cur_id) = cur_id {
                let char = self.characters.get_other_character_by_id_mut(&cur_id);
                char.reusable_core
                    .core
                    .hooked_character
                    .attached_characters_ids
                    .remove(&cur_id);
            }
            // if the new id is attaching to a player, add it to their list
            if let Some(id) = id {
                let char = self.characters.get_other_character_by_id_mut(&id);
                char.reusable_core
                    .core
                    .hooked_character
                    .attached_characters_ids
                    .insert(*self.character_id);
            }
        }
    }

    enum CannotMove {
        Left = 1 << 0,
        Right = 1 << 1,
        Up = 1 << 2,
        Down = 1 << 3,
    }

    const fn physical_size() -> f32 {
        28.0
    }
    const fn physical_size_vec2() -> vec2 {
        vec2 { x: 28.0, y: 28.0 }
    }

    impl Core {
        fn clamp_vel(move_restriction: i32, vel_param: &vec2) -> vec2 {
            let mut vel = vel_param.clone();
            if vel.x > 0.0 && (move_restriction & CannotMove::Right as i32) != 0 {
                vel.x = 0.0;
            }
            if vel.x < 0.0 && (move_restriction & CannotMove::Left as i32) != 0 {
                vel.x = 0.0;
            }
            if vel.y > 0.0 && (move_restriction & CannotMove::Down as i32) != 0 {
                vel.y = 0.0;
            }
            if vel.y < 0.0 && (move_restriction & CannotMove::Up as i32) != 0 {
                vel.y = 0.0;
            }
            return vel;
        }

        fn saturated_add<T: AddAssign + PartialOrd + num::traits::Zero>(
            min_val: T,
            max_val: T,
            current: T,
            modifier: T,
        ) -> T {
            let mut cur = current;
            if modifier < T::zero() {
                if cur < min_val {
                    return cur;
                }
                cur += modifier;
                if cur < min_val {
                    cur = min_val;
                }
                return cur;
            } else {
                if cur > max_val {
                    return cur;
                }
                cur += modifier;
                if cur > max_val {
                    cur = max_val;
                }
                return cur;
            }
        }

        fn get_gravity(core: &Core) -> f32 {
            core.tuning.gravity
        }

        pub fn physics_tick(
            &mut self,
            use_input: bool,
            do_deferred_tick: bool,
            pipe: &mut CorePipe,
            collision: &Collision,
        ) {
            let MsgObjPlayerInput {
                cursor,
                dir,
                jump,
                hook,
                ..
            } = &pipe.input.inp;
            self.move_restrictions = 0; // TODO core.m_pCollision->GetMoveRestrictions(UseInput ? IsSwitchActiveCb : 0, this, core.m_Pos);
            self.triggered_events = 0;

            // get ground state
            let grounded: bool = collision.check_point(
                self.pos.x + physical_size() / 2.0,
                self.pos.y + physical_size() / 2.0 + 5.0,
            ) || collision.check_point(
                self.pos.x - physical_size() / 2.0,
                self.pos.y + physical_size() / 2.0 + 5.0,
            );
            let cursor_vec = cursor.to_vec2();
            let cursor = vec2::new(cursor_vec.x as f32, cursor_vec.y as f32);
            let target_direction: vec2 = normalize(&cursor);

            self.vel.y += Self::get_gravity(self);

            let max_speed = if grounded {
                self.tuning.ground_control_speed
            } else {
                self.tuning.air_control_speed
            };
            let accel = if grounded {
                self.tuning.ground_control_accel
            } else {
                self.tuning.air_control_accel
            };
            let friction = if grounded {
                self.tuning.ground_friction
            } else {
                self.tuning.air_friction
            };

            // handle input
            if use_input {
                self.direction = **dir;

                // setup angle
                let tmp_angle = (target_direction.x).atan2(target_direction.y as f32);
                if tmp_angle < -(PI / 2.0) {
                    self.angle = ((tmp_angle + (2.0 * PI)) * 256.0) as i32;
                } else {
                    self.angle = (tmp_angle * 256.0) as i32;
                }

                // Special jump cases:
                // self.jumped == -1: A tee may only make one ground jump. Second jumped bit is always set
                // self.jumped == 0: A tee may not make a jump. Second jumped bit is always set
                // self.jumped == 1: A tee may do either a ground jump or an air jump. Second jumped bit is set after the first jump
                // The second jumped bit can be overridden by special tiles so that the tee can nevertheless jump.

                // handle jump
                if **jump {
                    if (self.jumped & 1) == 0 {
                        if grounded && ((self.jumped & 2) == 0 || self.jumps != 0) {
                            self.triggered_events |= CoreEvent::GroundJump as i32;
                            self.vel.y = -self.tuning.ground_jump_impulse;
                            if self.jumps > 1 {
                                self.jumped |= 1;
                            } else {
                                self.jumped |= 3;
                            }
                            self.jumped_total = 0;
                        } else if (self.jumped & 2) == 0 {
                            self.triggered_events |= CoreEvent::AirJump as i32;
                            self.vel.y = -self.tuning.air_jump_impulse;
                            self.jumped |= 3;
                            self.jumped_total += 1;
                        }
                    }
                } else {
                    self.jumped &= !1;
                }

                // handle hook
                if **hook {
                    if self.hook_state == HookState::Idle {
                        self.hook_state = HookState::HookFlying;
                        self.hook_pos = self.pos + target_direction * physical_size() * 1.5;
                        self.hook_dir = target_direction;
                        pipe.set_or_reset_hooked_char(None);
                        self.hook_tick =
                            (TICKS_PER_SECOND as f32 * (1.25 - self.tuning.hook_duration)) as i32;
                        self.triggered_events |= CoreEvent::HookLaunch as i32;
                    }
                } else {
                    pipe.set_or_reset_hooked_char(None);
                    self.hook_state = HookState::Idle;
                    self.hook_pos = self.pos;
                }
            }

            // handle jumping
            // 1 bit = to keep track if a jump has been made on this input (character is holding space bar)
            // 2 bit = to track if all air-jumps have been used up (tee gets dark feet)
            if grounded {
                self.jumped &= !2;
                self.jumped_total = 0;
            }

            // add the speed modification according to players wanted direction
            if self.direction < 0 {
                self.vel.x = Self::saturated_add(-max_speed, max_speed, self.vel.x, -accel);
            }
            if self.direction > 0 {
                self.vel.x = Self::saturated_add(-max_speed, max_speed, self.vel.x, accel);
            }
            if self.direction == 0 {
                self.vel.x *= friction;
            }

            // do hook
            if self.hook_state == HookState::Idle {
                pipe.set_or_reset_hooked_char(None);
                self.hook_pos = self.pos;
            } else if self.hook_state >= HookState::RetractStart
                && self.hook_state < HookState::RetractEnd
            {
                self.hook_state = HookState::from_i32(self.hook_state as i32 + 1).unwrap();
            } else if self.hook_state == HookState::RetractEnd {
                self.triggered_events |= CoreEvent::HookRetract as i32;
                self.hook_state = HookState::Retracted;
            } else if self.hook_state == HookState::HookFlying {
                let mut new_pos = self.hook_pos + self.hook_dir * self.tuning.hook_fire_speed;
                if (!self.new_hook && distance(&self.pos, &new_pos) > self.tuning.hook_length)
                    || (self.new_hook
                        && distance(&self.hook_tele_base, &new_pos) > self.tuning.hook_length)
                {
                    self.hook_state = HookState::RetractStart;
                    new_pos = self.pos + normalize(&(new_pos - self.pos)) * self.tuning.hook_length;
                    self.reset = true;
                }

                // make sure that the hook doesn't go though the ground
                let mut going_to_hit_ground = false;
                let mut going_to_retract = false;
                let mut going_through_tele = false;
                let mut tele_nr = 0;
                let mut before_col = vec2::default();
                let pos_1 = new_pos;
                let hook_pos = self.hook_pos;
                let hit = collision.intersect_line_tele_hook(
                    &hook_pos,
                    &pos_1,
                    &mut new_pos,
                    &mut before_col,
                    &mut tele_nr,
                );

                // self.m_NewHook = false;

                if hit > 0 {
                    if hit == TileNum::NoHook as i32 {
                        going_to_retract = true;
                    } else if hit == TileNum::TeleInHook as i32 {
                        going_through_tele = true;
                    } else {
                        going_to_hit_ground = true;
                    }
                    self.reset = true;
                }

                // Check against other players first
                if !self.hook_hit_disabled && self.tuning.player_hooking > 0.0 {
                    let mut distance_hook = 0.0;
                    let (
                        hook_pos,
                        mut hooked_player,
                        mut triggered_events,
                        mut hook_state,
                        is_super,
                        solo,
                    ) = (
                        self.hook_pos,
                        pipe.reusable_core
                            .hooked_character
                            .get_hooked_char()
                            .clone(),
                        self.triggered_events,
                        self.hook_state,
                        self.is_super,
                        self.solo,
                    );
                    pipe.get_other_character_id_and_cores_iter_mut(&mut |char_id, char_core, _| {
                        if !(is_super || char_core.is_super) && (char_core.solo || solo) {
                            return;
                        }

                        let mut closest_point: vec2 = vec2::default();
                        if closest_point_on_line(
                            &hook_pos,
                            &new_pos,
                            &char_core.pos,
                            &mut closest_point,
                        ) {
                            if distance(&char_core.pos, &closest_point) < physical_size() + 2.0 {
                                if hooked_player.is_none()
                                    || distance(&hook_pos, &char_core.pos) < distance_hook
                                {
                                    triggered_events |= CoreEvent::HookAttachPlayer as i32;
                                    hook_state = HookState::HookGrabbed;
                                    hooked_player = Some(char_id.clone());
                                    distance_hook = distance(&hook_pos, &char_core.pos);
                                }
                            }
                        }
                    });
                    pipe.set_or_reset_hooked_char(hooked_player);
                    self.triggered_events = triggered_events;
                    self.hook_state = hook_state;
                }

                if self.hook_state == HookState::HookFlying {
                    // check against ground
                    if going_to_hit_ground {
                        self.triggered_events |= CoreEvent::HookAttachGround as i32;
                        self.hook_state = HookState::HookGrabbed;
                    } else if going_to_retract {
                        self.triggered_events |= CoreEvent::HookHitNoHook as i32;
                        self.hook_state = HookState::RetractStart;
                    }

                    if going_through_tele
                    /* TODO: && self.m_pTeleOuts && !self.m_pTeleOuts->empty() && !(*self.m_pTeleOuts)[teleNr - 1].empty()*/
                    {
                        self.triggered_events = 0;
                        pipe.set_or_reset_hooked_char(None);
                        self.new_hook = true;
                        let _random_out = 0; // TODO: self.m_pWorld->RandomOr0((*self.m_pTeleOuts)[teleNr - 1].size());
                        self.hook_pos = vec2::default(); // TODO: (*self.m_pTeleOuts)[teleNr - 1][RandomOut] + TargetDirection * PhysicalSize() * 1.5f;
                        self.hook_dir = target_direction;
                        self.hook_tele_base = self.hook_pos;
                    } else {
                        self.hook_pos = new_pos;
                    }
                }
            }

            if self.hook_state == HookState::HookGrabbed {
                if let Some(hooked_char_id) = pipe
                    .reusable_core
                    .hooked_character
                    .get_hooked_char()
                    .clone()
                {
                    let char_core = pipe.get_other_character_core_by_id(&hooked_char_id);
                    let other_char_pos = char_core.pos;
                    self.hook_pos = other_char_pos;
                }

                // don't do this hook rutine when we are hook to a character
                if pipe
                    .reusable_core
                    .hooked_character
                    .get_hooked_char()
                    .is_none()
                    && distance(&self.hook_pos, &self.pos) > 46.0
                {
                    let mut hook_vel =
                        normalize(&(self.hook_pos - self.pos)) * self.tuning.hook_drag_accel;
                    // the hook as more power to drag you up then down.
                    // this makes it easier to get on top of an platform
                    if hook_vel.y > 0.0 {
                        hook_vel.y *= 0.3;
                    }

                    // the hook will boost it's power if the character wants to move
                    // in that direction. otherwise it will dampen everything abit
                    if (hook_vel.x < 0.0 && self.direction < 0)
                        || (hook_vel.x > 0.0 && self.direction > 0)
                    {
                        hook_vel.x *= 0.95;
                    } else {
                        hook_vel.x *= 0.75;
                    }

                    let new_vel = self.vel + hook_vel;

                    // check if we are under the legal limit for the hook
                    if length(&new_vel) < self.tuning.hook_drag_speed
                        || length(&new_vel) < length(&self.vel)
                    {
                        self.vel = new_vel; // no problem. apply
                    }
                }

                // release hook (max default hook time is 1.25 s)
                self.hook_tick += 1;
                if let Some(_) = pipe.reusable_core.hooked_character.get_hooked_char() {
                    if self.hook_tick > TICKS_PER_SECOND as i32 + TICKS_PER_SECOND as i32 / 5 {
                        pipe.set_or_reset_hooked_char(None);
                        self.hook_state = HookState::Retracted;
                        self.hook_pos = self.pos;
                    }
                }
            }

            if do_deferred_tick {
                self.physics_tick_deferred(pipe);
            }
        }

        pub fn physics_tick_deferred(&mut self, pipe: &mut CorePipe) {
            let (
                is_super,
                solo,
                collision_disabled,
                tuning,
                hook_hit_disabled,
                pos,
                vel,
                move_restrictions,
            ) = (
                self.is_super,
                self.solo,
                self.collision_disabled,
                self.tuning,
                self.hook_hit_disabled,
                self.pos,
                &mut self.vel,
                self.move_restrictions,
            );
            let hooked_player = pipe
                .reusable_core
                .hooked_character
                .get_hooked_char()
                .clone();

            pipe.get_other_character_id_and_cores_iter_mut(&mut |char_id, char_core, _| {
                if !(is_super || char_core.is_super) && (solo || char_core.solo) {
                    return;
                }

                // handle character <-> character collision
                let distance_pos = distance(&pos, &char_core.pos);
                if distance_pos > 0.0 {
                    let dir = normalize(&(pos - char_core.pos));

                    let can_collide = (is_super || char_core.is_super)
                        || (!collision_disabled
                            && !char_core.collision_disabled
                            && tuning.player_collision > 0.0);

                    if can_collide && distance_pos < physical_size() * 1.25 && distance_pos > 0.0 {
                        let a = physical_size() * 1.45 - distance_pos;
                        let mut velocity = 0.5;

                        // make sure that we don't add excess force by checking the
                        // direction against the current velocity. if not zero.
                        if length(&vel) > 0.0001 {
                            velocity = 1.0 - (dot(&normalize(&vel), &dir) + 1.0) / 2.0;
                            // TODO: Wdouble-promotion don't fix this as this might change game physics
                        }

                        *vel += dir * a * (velocity * 0.75);
                        *vel *= 0.85;
                    }

                    // handle hook influence
                    if !hook_hit_disabled
                        && hooked_player == Some(char_id.clone())
                        && tuning.player_hooking > 0.0
                    {
                        if distance_pos > physical_size() * 1.50
                        // TODO: fix tweakable variable
                        {
                            let hook_accel =
                                tuning.hook_drag_accel * (distance_pos / tuning.hook_length);
                            let drag_speed = tuning.hook_drag_speed;

                            let mut temp = vec2::default();
                            // add force to the hooked character
                            temp.x = Self::saturated_add(
                                -drag_speed,
                                drag_speed,
                                char_core.vel.x,
                                hook_accel * dir.x * 1.5,
                            );
                            temp.y = Self::saturated_add(
                                -drag_speed,
                                drag_speed,
                                char_core.vel.y,
                                hook_accel * dir.y * 1.5,
                            );
                            char_core.vel = Self::clamp_vel(char_core.move_restrictions, &temp);
                            // add a little bit force to the guy who has the grip
                            temp.x = Self::saturated_add(
                                -drag_speed,
                                drag_speed,
                                vel.x,
                                -hook_accel * dir.x * 0.25,
                            );
                            temp.y = Self::saturated_add(
                                -drag_speed,
                                drag_speed,
                                vel.y,
                                -hook_accel * dir.y * 0.25,
                            );
                            *vel = Self::clamp_vel(move_restrictions, &temp);
                        }
                    }
                }
            });

            if self.hook_state != HookState::HookFlying {
                self.new_hook = false;
            }

            // clamp the velocity to something sane
            if length(&self.vel) > 6000.0 {
                self.vel = normalize(&self.vel) * 6000.0;
            }
        }

        fn velocity_ramp(value: f32, start: f32, range: f32, curvature: f32) -> f32 {
            if value < start {
                return 1.0;
            }
            return 1.0 / curvature.powf((value - start) / range);
        }

        pub fn physics_move(&mut self, pipe: &mut CorePipe, collision: &Collision) {
            let ramp_value = Self::velocity_ramp(
                length(&self.vel) * 50.0,
                self.tuning.velramp_start,
                self.tuning.velramp_range,
                self.tuning.velramp_curvature,
            );

            self.vel.x = self.vel.x * ramp_value;

            let mut new_pos = self.pos;

            let old_vel = self.vel;
            collision.move_box(&mut new_pos, &mut self.vel, &physical_size_vec2(), 0.0);

            self.colliding = 0;
            if self.vel.x < 0.001 && self.vel.x > -0.001 {
                if old_vel.x > 0.0 {
                    self.colliding = 1;
                } else if old_vel.x < 0.0 {
                    self.colliding = 2;
                }
            } else {
                self.left_wall = true;
            }

            self.vel.x = self.vel.x * (1.0 / ramp_value);

            if self.is_super
                || (self.tuning.player_collision > 0.0 && !self.collision_disabled && !self.solo)
            {
                // check character collision
                let distance_pos = distance(&self.pos, &new_pos);
                if distance_pos > 0.0 {
                    let end = distance_pos + 1.0;
                    let mut last_pos = self.pos;
                    for i in 0..end as i32 {
                        let a = i as f32 / distance_pos;
                        let pos = mix(&self.pos, &new_pos, a);
                        let (is_super, solo, mut core_pos) = (self.is_super, self.solo, self.pos);
                        pipe.get_other_character_id_and_cores_iter(&mut |_, char_core| {
                            if !(char_core.is_super || is_super)
                                && (solo || char_core.solo || char_core.collision_disabled)
                            {
                                return;
                            }
                            let d = distance(&pos, &char_core.pos);
                            if d < physical_size() && d >= 0.0 {
                                if a > 0.0 {
                                    core_pos = last_pos;
                                } else if distance(&new_pos, &char_core.pos) > d {
                                    core_pos = new_pos;
                                }
                                return;
                            }
                        });
                        self.pos = core_pos;
                        last_pos = pos;
                    }
                }
            }

            self.pos = new_pos;
        }

        /*
        pub fn ReadDDNet(const CNetObj_DDNetCharacter *pObjDDNet)
        {
        // Collision
        core.m_Solo = pObjDDNet->core.m_Flags & CHARACTERFLAG_SOLO;
        core.m_Jetpack = pObjDDNet->core.m_Flags & CHARACTERFLAG_JETPACK;
        core.m_CollisionDisabled = pObjDDNet->core.m_Flags & CHARACTERFLAG_COLLISION_DISABLED;
        core.m_HammerHitDisabled = pObjDDNet->core.m_Flags & CHARACTERFLAG_HAMMER_HIT_DISABLED;
        core.m_ShotgunHitDisabled = pObjDDNet->core.m_Flags & CHARACTERFLAG_SHOTGUN_HIT_DISABLED;
        core.m_GrenadeHitDisabled = pObjDDNet->core.m_Flags & CHARACTERFLAG_GRENADE_HIT_DISABLED;
        core.m_LaserHitDisabled = pObjDDNet->core.m_Flags & CHARACTERFLAG_LASER_HIT_DISABLED;
        core.m_HookHitDisabled = pObjDDNet->core.m_Flags & CHARACTERFLAG_HOOK_HIT_DISABLED;
        core.m_Super = pObjDDNet->core.m_Flags & CHARACTERFLAG_SUPER;

        // Endless
        core.m_EndlessHook = pObjDDNet->core.m_Flags & CHARACTERFLAG_ENDLESS_HOOK;
        core.m_EndlessJump = pObjDDNet->core.m_Flags & CHARACTERFLAG_ENDLESS_JUMP;

        // Freeze
        core.m_FreezeEnd = pObjDDNet->core.m_FreezeEnd;
        core.m_DeepFrozen = pObjDDNet->core.m_FreezeEnd == -1;
        core.m_LiveFrozen = (pObjDDNet->core.m_Flags & CHARACTERFLAG_MOVEMENTS_DISABLED) != 0;

        // Telegun
        core.m_HasTelegunGrenade = pObjDDNet->core.m_Flags & CHARACTERFLAG_TELEGUN_GRENADE;
        core.m_HasTelegunGun = pObjDDNet->core.m_Flags & CHARACTERFLAG_TELEGUN_GUN;
        core.m_HasTelegunLaser = pObjDDNet->core.m_Flags & CHARACTERFLAG_TELEGUN_LASER;

        // Weapons
        core.m_aWeapons[WEAPON_HAMMER].m_Got = (pObjDDNet->core.m_Flags & CHARACTERFLAG_WEAPON_HAMMER) != 0;
        core.m_aWeapons[WEAPON_GUN].m_Got = (pObjDDNet->core.m_Flags & CHARACTERFLAG_WEAPON_GUN) != 0;
        core.m_aWeapons[WEAPON_SHOTGUN].m_Got = (pObjDDNet->core.m_Flags & CHARACTERFLAG_WEAPON_SHOTGUN) != 0;
        core.m_aWeapons[WEAPON_GRENADE].m_Got = (pObjDDNet->core.m_Flags & CHARACTERFLAG_WEAPON_GRENADE) != 0;
        core.m_aWeapons[WEAPON_LASER].m_Got = (pObjDDNet->core.m_Flags & CHARACTERFLAG_WEAPON_LASER) != 0;
        core.m_aWeapons[WEAPON_NINJA].m_Got = (pObjDDNet->core.m_Flags & CHARACTERFLAG_WEAPON_NINJA) != 0;

        // Available jumps
        core.m_Jumps = pObjDDNet->core.m_Jumps;

        // Display Information
        // We only accept the display information when it is received, which means it is not -1 in each case.
        if(pObjDDNet->core.m_JumpedTotal != -1)
        {
            core.m_JumpedTotal = pObjDDNet->core.m_JumpedTotal;
        }
        if(pObjDDNet->core.m_NinjaActivationTick != -1)
        {
            core.m_Ninja.m_ActivationTick = pObjDDNet->core.m_NinjaActivationTick;
        }
        if(pObjDDNet->core.m_FreezeStart != -1)
        {
            core.m_FreezeStart = pObjDDNet->core.m_FreezeStart;
            core.m_IsInFreeze = pObjDDNet->core.m_Flags & CHARACTERFLAG_IN_FREEZE;
        }
        }*/

        pub fn physics_quantize(&mut self) {
            let mut net_core = NetObjCharacterCore::default();
            self.physics_write(&mut net_core);
            self.physics_read(&net_core);
        }
    }
    /*
    // DDRace

    void CCharacterCore::SetTeamsCore(CTeamsCore *pTeams)
    {
    core.m_pTeams = pTeams;
    }

    void CCharacterCore::SetTeleOuts(std::map<int, std::vector<vec2>> *pTeleOuts)
    {
    core.m_pTeleOuts = pTeleOuts;
    }

    bool CCharacterCore::IsSwitchActiveCb(int Number, void *pUser)
    {
    CCharacterCore *pThis = (CCharacterCore *)pUser;
    if(pThis->core.m_pWorld && !pThis->core.m_pWorld->core.m_vSwitchers.empty())
        if(pThis->core.m_Id != -1 && pThis->core.m_pTeams->Team(pThis->core.m_Id) != (pThis->core.m_pTeams->core.m_IsDDRace16 ? VANILLA_TEAcore.m_SUPER : TEAcore.m_SUPER))
            return pThis->core.m_pWorld->core.m_vSwitchers[Number].m_aStatus[pThis->core.m_pTeams->Team(pThis->core.m_Id)];
    return false;
    }

    void CWorldCore::InitSwitchers(int HighestSwitchNumber)
    {
    if(HighestSwitchNumber > 0)
        core.m_vSwitchers.resize(HighestSwitchNumber + 1);
    else
        core.m_vSwitchers.clear();

    for(auto &Switcher : core.m_vSwitchers)
    {
        Switcher.m_Initial = true;
        for(int j = 0; j < MAX_CLIENTS; j++)
        {
            Switcher.m_aStatus[j] = true;
            Switcher.m_aEndTick[j] = 0;
            Switcher.m_aType[j] = 0;
            Switcher.m_aLastUpdateTick[j] = 0;
        }
    }
    }
    */
}
