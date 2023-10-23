use std::ops::{Deref, DerefMut};

use arrayvec::{ArrayString, CapacityError};

use math::math::vector::{dvec2, vec2, vec4_base};
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

use crate::{game_types::TGameElementID, types::NetFloatIntegerRepType};

use bincode::{BorrowDecode, Decode, Encode};

use super::types::{
    chat::{NetChatMsg, NetMsgSystem},
    killfeed::NetKillfeedMsg,
};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct NetworkStr<const CAP: usize>(#[bincode(with_serde)] ArrayString<CAP>);

impl<const CAP: usize> NetworkStr<CAP> {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }

    pub fn from(s: &str) -> Result<Self, CapacityError<&str>> {
        let arrstr = ArrayString::from(s)?;
        Ok(NetworkStr(arrstr))
    }
}

// # server -> client
#[derive(Clone, Serialize, Deserialize, Decode, Encode)]
pub struct MsgObjPlayerInfo {
    pub name: NetworkStr<{ 15 * 4 }>,
    pub clan: NetworkStr<{ 10 * 4 }>,
    pub country: NetworkStr<3>,

    // skin
    pub skin_body: MsgObjGameSkinPartInfo,
    pub skin_ears: MsgObjGameSkinPartInfo,
    pub skin_feet: MsgObjGameSkinPartInfo,
    pub skin_hand: MsgObjGameSkinPartInfo,
    pub skin_decoration: MsgObjGameSkinPartInfo,

    pub skin_animation_name: NetworkStr<{ 24 * 4 }>,

    pub skin_permanent_effect_name: NetworkStr<{ 24 * 4 }>,
    pub skin_state_effects_name: NetworkStr<{ 24 * 4 }>,
    pub skin_server_state_effects_name: NetworkStr<{ 24 * 4 }>,
    pub skin_status_effects_name: NetworkStr<{ 24 * 4 }>,

    pub pistol: MsgObjGameWeaponInfo,
    pub grenade: MsgObjGameWeaponInfo,
    pub laser: MsgObjGameWeaponInfo,
    pub puller: MsgObjGameWeaponInfo,
    pub shotgun: MsgObjGameWeaponInfo,
    pub hammer: MsgObjGameWeaponInfo,
    pub ninja: MsgObjGameWeaponInfo,
}

impl MsgObjPlayerInfo {
    // only provide a default that makes clear you used default
    pub fn explicit_default() -> Self {
        Self {
            name: NetworkStr::from("TODO").unwrap(),
            clan: NetworkStr::from("TODO").unwrap(),
            country: NetworkStr::from("GER").unwrap(),
            skin_body: MsgObjGameSkinPartInfo {
                name: NetworkStr::from("TODO").unwrap(),
                color_swizzle_r: ColorChannel::R,
                color_swizzle_g: ColorChannel::G,
                color_swizzle_b: ColorChannel::B,
                color_swizzle_a: ColorChannel::A,
                color: vec4_base::<u8> {
                    x: 255,
                    y: 255,
                    z: 255,
                    w: 255,
                },
            },
            skin_ears: MsgObjGameSkinPartInfo {
                name: NetworkStr::from("TODO").unwrap(),
                color_swizzle_r: ColorChannel::R,
                color_swizzle_g: ColorChannel::G,
                color_swizzle_b: ColorChannel::B,
                color_swizzle_a: ColorChannel::A,
                color: vec4_base::<u8> {
                    x: 255,
                    y: 255,
                    z: 255,
                    w: 255,
                },
            },
            skin_feet: MsgObjGameSkinPartInfo {
                name: NetworkStr::from("TODO").unwrap(),
                color_swizzle_r: ColorChannel::R,
                color_swizzle_g: ColorChannel::G,
                color_swizzle_b: ColorChannel::B,
                color_swizzle_a: ColorChannel::A,
                color: vec4_base::<u8> {
                    x: 255,
                    y: 255,
                    z: 255,
                    w: 255,
                },
            },
            skin_hand: MsgObjGameSkinPartInfo {
                name: NetworkStr::from("TODO").unwrap(),
                color_swizzle_r: ColorChannel::R,
                color_swizzle_g: ColorChannel::G,
                color_swizzle_b: ColorChannel::B,
                color_swizzle_a: ColorChannel::A,
                color: vec4_base::<u8> {
                    x: 255,
                    y: 255,
                    z: 255,
                    w: 255,
                },
            },
            skin_decoration: MsgObjGameSkinPartInfo {
                name: NetworkStr::from("TODO").unwrap(),
                color_swizzle_r: ColorChannel::R,
                color_swizzle_g: ColorChannel::G,
                color_swizzle_b: ColorChannel::B,
                color_swizzle_a: ColorChannel::A,
                color: vec4_base::<u8> {
                    x: 255,
                    y: 255,
                    z: 255,
                    w: 255,
                },
            },
            skin_animation_name: NetworkStr::from("TODO").unwrap(),
            skin_permanent_effect_name: NetworkStr::from("TODO").unwrap(),
            skin_state_effects_name: NetworkStr::from("TODO").unwrap(),
            skin_server_state_effects_name: NetworkStr::from("TODO").unwrap(),
            skin_status_effects_name: NetworkStr::from("TODO").unwrap(),
            pistol: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
            grenade: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
            laser: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
            puller: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
            shotgun: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
            hammer: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
            ninja: MsgObjGameWeaponInfo {
                name: NetworkStr::from("TODO").unwrap(),
                anim_name: NetworkStr::from("TODO").unwrap(),
                effect_name: NetworkStr::from("TODO").unwrap(),
            },
        }
    }
}

const MAX_MAP_NAME_LEN: usize = 64;
const MAX_GAME_TYPE_NAME_LEN: usize = 32;
/**
 * All information about the server
 * so that the client can prepare the game.
 * E.g. current map
 */
#[derive(Clone, Serialize, Deserialize, Decode, Encode)]
pub struct MsgSvServerInfo {
    /// the map that is currently played on
    pub map: NetworkStr<MAX_MAP_NAME_LEN>,
    /// as soon as the client has finished loading it might want to render something to the screen
    /// the server can give a hint what the best camera position is for that
    pub hint_start_camera_pos: vec2,
    /// the game type currently played
    pub game_type: NetworkStr<MAX_GAME_TYPE_NAME_LEN>,
}

#[derive(Serialize, Deserialize, Decode, Encode, Clone)]
pub struct MsgSvPlayerInfo {
    pub id: TGameElementID,
    pub info: MsgObjPlayerInfo,
    pub version: u64,
}

#[derive(Serialize, Deserialize, Decode, Encode, Clone)]
pub struct MsgSvChatMsg {
    pub msg: NetChatMsg,
}

#[derive(Serialize, Deserialize, Decode, Encode, Clone)]
pub struct MsgSvSystemMsg {
    pub msg: NetMsgSystem,
}

#[derive(Serialize, Deserialize, Decode, Encode, Clone)]
pub struct MsgSvKillfeedMsg {
    pub msg: NetKillfeedMsg,
}

// # client message parts
#[derive(Clone, Serialize, Deserialize, Decode, Encode)]
pub enum ColorChannel {
    R = 0,
    G,
    B,
    A,
}

#[derive(Clone, Serialize, Deserialize, Decode, Encode)]
pub struct MsgObjGameSkinPartInfo {
    pub name: NetworkStr<{ 24 * 4 }>,
    pub color_swizzle_r: ColorChannel,
    pub color_swizzle_g: ColorChannel,
    pub color_swizzle_b: ColorChannel,
    pub color_swizzle_a: ColorChannel,
    pub color: vec4_base<u8>,
}

#[derive(Clone, Serialize, Deserialize, Decode, Encode)]
pub struct MsgObjGameWeaponInfo {
    pub name: NetworkStr<{ 24 * 4 }>,
    pub anim_name: NetworkStr<{ 24 * 4 }>,
    pub effect_name: NetworkStr<{ 24 * 4 }>,
}

// # client -> server
// TODO: move this somewhere better
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    FromPrimitive,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    Hash,
    PartialOrd,
    Ord,
)]
pub enum WeaponType {
    #[default]
    Hammer = 0,
    Gun,
    Shotgun,
    Grenade,
    Laser,
    Ninja,
}
pub const NUM_WEAPONS: usize = WeaponType::Ninja as usize + 1;

#[derive(Encode, Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub struct MsgObjPlayerInputCursor {
    pub x: NetFloatIntegerRepType,
    pub y: NetFloatIntegerRepType,
}

impl Default for MsgObjPlayerInputCursor {
    fn default() -> Self {
        Self {
            x: 1,
            y: Default::default(),
        }
    }
}

impl Decode for MsgObjPlayerInputCursor {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        let mut res = bincode::serde::decode_from_reader::<Self, _, _>(decoder.reader(), conf)?;
        if res.x == 0 && res.y == 0 {
            // TODO: handle this here?
            res.x = 1;
        }
        Ok(res)
    }
}

impl<'de> BorrowDecode<'de> for MsgObjPlayerInputCursor {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Self::decode(decoder)
    }
}

impl MsgObjPlayerInputCursor {
    pub fn to_vec2(&self) -> dvec2 {
        dvec2::new(self.x as f64 / 10000.0, self.y as f64 / 10000.0)
    }
    pub fn from_vec2(&mut self, cursor: &dvec2) {
        self.x = (cursor.x * 10000.0) as i64;
        self.y = (cursor.y * 10000.0) as i64;
    }
}

#[derive(Decode, Encode, Copy, Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputVarAchsis<V> {
    pub val: V,
    // the version increases by moving the achsis
    pub change_version: u64,
}

impl<V: PartialEq> InputVarAchsis<V> {
    pub fn set(&mut self, val: V) {
        if val != self.val {
            self.val = val;
            self.change_version += 1;
        }
    }
}

impl<V> Deref for InputVarAchsis<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<V> DerefMut for InputVarAchsis<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

#[derive(Decode, Encode, Copy, Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputVarClickable<V> {
    pub val: V,
    // these values increase by clicking or releasing the key
    pub clicks: u64,
    pub releases: u64,
}

pub trait IsClickedBool {
    fn is_true(&self) -> bool;
}

impl IsClickedBool for bool {
    fn is_true(&self) -> bool {
        *self
    }
}

impl<T> IsClickedBool for Option<T> {
    fn is_true(&self) -> bool {
        self.is_some()
    }
}

impl<V: PartialEq> InputVarClickable<V>
where
    V: IsClickedBool,
{
    pub fn set(&mut self, val: V) {
        if val != self.val {
            if !val.is_true() {
                self.releases += 1;
            } else {
                self.clicks += 1;
            }
            self.val = val;
        }
    }

    pub fn is_currently_clicked(&self) -> bool {
        self.is_true()
    }
}

impl<V: PartialEq> InputVarClickable<V> {
    pub fn set_ex(&mut self, val: V, was_released: bool) {
        if val != self.val {
            self.val = val;
            if was_released {
                self.releases += 1;
            } else {
                self.clicks += 1;
            }
        }
    }

    pub fn was_clicked(&self, old: &Self) -> bool {
        if self.clicks > old.clicks {
            true
        } else {
            false
        }
    }
}

impl<V> Deref for InputVarClickable<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<V> DerefMut for InputVarClickable<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

#[derive(Decode, Encode, Copy, Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsgObjPlayerInput {
    pub dir: InputVarAchsis<i32>,
    pub cursor: InputVarAchsis<MsgObjPlayerInputCursor>,

    pub jump: InputVarClickable<bool>,
    pub fire: InputVarClickable<bool>,
    pub hook: InputVarClickable<bool>,
    pub flags: InputVarClickable<i32>,
    pub weapon_req: InputVarClickable<Option<WeaponType>>,
    // TODO: next prev weapon, needed?
    pub weapon_diff: InputVarAchsis<i32>,
}

#[derive(Decode, Encode)]
pub struct MsgClReady {
    pub player_info: MsgObjPlayerInfo,
}

#[derive(Decode, Encode)]
pub struct MsgClAddLocalPlayer {
    pub player_info: MsgObjPlayerInfo,
}

#[derive(Debug, Decode, Encode)]
pub struct MsgClInput {
    pub version: u64,
    pub inp: MsgObjPlayerInput,
}

#[derive(Debug, Decode, Encode)]
pub enum MsgClChatMsg {
    Global {
        msg: NetworkStr<256>,
    },
    GameTeam {
        team: u32, // TODO
        msg: NetworkStr<256>,
    },
    Whisper {
        receiver_id: TGameElementID,
        msg: NetworkStr<256>,
    },
}
/*
// # client -> server
struct msg_cl_global_say {
    client_id: ClientID,
    msg: [u8; 256 * 4], // 256 utf-8 characters
}

struct msg_cl_team_say {
    client_id: ClientID,
    msg: [u8; 256 * 4], // 256 utf-8 characters
}

struct msg_cl_whisper {
    client_id: ClientID,
    target_ids: Vec<ClientID>, // TODO
    msg: [u8; 256 * 4],        // 256 utf-8 characters
}

struct msg_cl_set_stage {
    stage_index: u32,
}

enum GameTeams {
    Red = 0,
    Blue,
    Spectator,
}

struct msg_cl_set_team {
    team_index: GameTeams,
}

enum SpectateModes {
    FreeCam((NetFloatIntegerRepType, NetFloatIntegerRepType)), // x and y coords
    Flag(u32),
    Player(u32),
}

struct msg_cl_set_spectator {
    mode: SpectateModes,
}

// TODO split this package in smaller ones, only sent non "default" (e.g. if skin name is "default" simply dont send it) packages
struct msg_cl_player_info {
    name: [u8; 15 * 4],
    clan: [u8; 10 * 4],
    country: [u8; 3], // only ansii chars for this

    // skin
    skin_body: game_skin_part_info,
    skin_ears: game_skin_part_info,
    skin_feet: game_skin_part_info,
    skin_hand: game_skin_part_info,
    skin_decoration: game_skin_part_info,

    skin_animation_name: [u8; 24 * 4],

    skin_permanent_effect_name: [u8; 24 * 4],
    skin_state_effects_name: [u8; 24 * 4],
    skin_server_state_effects_name: [u8; 24 * 4],
    skin_status_effects_name: [u8; 24 * 4],

    pistol: game_weapon_info,
    grenade: game_weapon_info,
    laser: game_weapon_info,
    puller: game_weapon_info,
    shotgun: game_weapon_info,
    hammer: game_weapon_info,
    ninja: game_weapon_info,
}

struct msg_cl_kill {}

struct msg_cl_ready {}

enum EmoticonGroup {
    Vanilla = 0,
}

struct msg_cl_emoticon {
    emoticon_group: EmoticonGroup,
    emoticon_index: u32,
}

struct msg_cl_vote {}

struct msg_cl_call_vote {}

// helper structs
struct msg_obj_player_input {
    dir: i32,
    cursor_x: NetFloatIntegerRepType,
    cursor_y: NetFloatIntegerRepType,

    jump: bool,
    fire: i32,
    hook: bool,
    flags: i32,
    weapon_req: WeaponType,
    // TODO: next prev weapon, needed?
    weapon_diff: i32,
}

struct msg_obj_proj {
    x: NetFloatIntegerRepType,
    y: NetFloatIntegerRepType,

    vel_x: NetFloatIntegerRepType,
    vel_y: NetFloatIntegerRepType,

    weapon_type: WeaponType,
    client_owner_id: ClientID,

    start_tick: GameTickType,
}

struct msg_obj_laser {
    x: NetFloatIntegerRepType,
    y: NetFloatIntegerRepType,
    start_x: NetFloatIntegerRepType,
    start_y: NetFloatIntegerRepType,

    weapon_type: WeaponType,
    client_owner_id: ClientID,

    start_tick: GameTickType,
}

struct msg_obj_pickup {
    x: NetFloatIntegerRepType,
    y: NetFloatIntegerRepType,

    start_tick: GameTickType,
}

enum GameTeam {
    // TODO
}

struct msg_obj_flag {
    x: NetFloatIntegerRepType,
    y: NetFloatIntegerRepType,

    team: GameTeam,
}

struct msg_obj_game_data {
    start_tick: GameTickType,
    state_flags: i32,
    state_end_tick: GameTickType,
}

struct msg_obj_game_team_data {
    team: GameTeam,
    score: i64,
}

struct msg_obj_game_flag_data {
    team: GameTeam,
    carrier_id: ClientID,
    drop_tick: GameTickType,
}

enum HookState {
    // TODO
}

struct msg_obj_game_core {
    tick: GameTickType,

    x: NetFloatIntegerRepType,
    y: NetFloatIntegerRepType,

    vel_x: NetFloatIntegerRepType,
    vel_y: NetFloatIntegerRepType,

    angle: NetFloatIntegerRepType,
    dir: i8,

    jumped: i32,
    hooked_player_id: ClientID,
    hook_state: HookState,
    hook_tick: GameTickType,

    hook_x: NetFloatIntegerRepType,
    hook_y: NetFloatIntegerRepType,
    hook_dx: NetFloatIntegerRepType,
    hook_dy: NetFloatIntegerRepType,
}

enum EmoteType {
    //
}

struct msg_obj_game_character_core {
    health: i32,
    armor: i32,
    ammo: i32,
    weapon: WeaponType,
    emote: EmoteType,
    attack_tick: GameTickType,
    triggered_events: i32,
}

struct msg_obj_game_player_info {
    flags: i32,
    score: i64,
    latency: u64,
}

struct msg_obj_game_spectator_info {
    spec_mode: SpectateModes,
    x: NetFloatIntegerRepType,
    y: NetFloatIntegerRepType,
}

struct msg_sv_motd {
    msg: [u8; 256 * 4], // 256 utf8 characters allowed
}

struct msg_sv_broadcast {
    msg: [u8; 256 * 4], // 256 utf8 characters allowed
}

struct msg_sv_team_chat {
    from: ClientID,
    msg: [u8; 256 * 4], // 256 utf8 characters allowed
}

struct msg_sv_chat {
    from: ClientID,
    msg: [u8; 256 * 4], // 256 utf8 characters allowed
}

struct msg_sv_whisper {
    from: ClientID,
    to: Vec<ClientID>,
    msg: [u8; 256 * 4], // 256 utf8 characters allowed
}

struct msg_sv_team {
    client_id: ClientID,
    team: GameTeam,
    silent: bool,
    cooldown_tick: GameTickType,
}

struct msg_sv_kill_msg {
    killer_ids: Vec<ClientID>,
    assist_ids: Vec<ClientID>,
    victim_ids: Vec<ClientID>,
    weapon: WeaponType,
    // TODO mode special?
}

struct tune {
    name: String,
    value: String,
}

struct msg_sv_tune_params {
    tunes: [tune],
}

struct msg_sv_weapon_pickup {
    weapon: WeaponType,
    ammo: i32,
}

struct msg_sv_emoticon {
    client_id: ClientID,
    emoticon_group: i32,
    emoticon_index: i32,
}

struct msg_sv_vote_clear_options {}

struct msg_sv_vote_list_add {}

struct msg_sv_vote_add {
    group: [u8; 128 * 4],       // utf-8 string. 128 chars allowed
    description: [u8; 128 * 4], // utf-8 string. 128 chars allowed
}

struct msg_sv_vote_rem {
    // TODO
}

struct msg_sv_vote_set {
    // TODO
}

struct msg_sv_vote_status {
    // TODO
}

struct msg_sv_server_settings {
    can_kick_vote: bool,
    pub can_spec_vote: bool,
}

struct msg_sv_client_info {
    pub client_id: ClientID,
    pub is_client_local_player: bool,
    pub team: GameTeam,
    pub name: [u8; 15 * 4], // todo share with client code
    pub clan: [u8; 15 * 4], // todo share with client code
    pub country: [u8; 3],   // todo share with client code
    // TODO share skin stuff and names with client code
    pub silent: bool, // TODO really needed?
}

struct msg_sv_game_info {
    game_flags: i32,
    score_limit: i32,
    time_limit: i32,
    match_num: u32,
    cur_match_index: u32,
}

struct msg_sv_client_drop {
    client_id: ClientID, // TODO: maybe add a general way to promote such messages, instead of adding so much packets
    reason: [u8; 256 * 4], // utf8 string. 256 characters allowed
    silent: bool,
}

struct msg_sv_game_msg {
    // TODO
}
*/
