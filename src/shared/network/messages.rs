use arrayvec::{ArrayString, CapacityError};

use math::math::vector::vec4_base;

use crate::{game::snapshot::Snapshot, types::NetFloatIntegerRepType};

use bincode::{Decode, Encode};

#[derive(Clone)]
pub struct NetworkStr<const CAP: usize>(ArrayString<CAP>);

impl<const CAP: usize> NetworkStr<CAP> {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }

    pub fn from(s: &str) -> Result<Self, CapacityError<&str>> {
        let arrstr = ArrayString::from(s)?;
        Ok(NetworkStr(arrstr))
    }
}

impl<const CAP: usize> Encode for NetworkStr<CAP> {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        bincode::encode_into_writer(
            self.as_str().to_string(),
            encoder.writer(),
            bincode::config::standard(),
        )
    }
}

impl<const CAP: usize> Decode for NetworkStr<CAP> {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let decoded: String =
            bincode::decode_from_reader(decoder.reader(), bincode::config::standard())?;
        let arr = ArrayString::from(decoded.as_str());
        match arr {
            Ok(res) => Ok(Self(res)),
            Err(_err) => Err(bincode::error::DecodeError::InvalidCharEncoding([0; 4])),
        }
    }
}

impl<'a, const CAP: usize> bincode::BorrowDecode<'a> for NetworkStr<CAP> {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'a>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let decoded: String =
            bincode::decode_from_reader(decoder.reader(), bincode::config::standard())?;
        let arr = ArrayString::from(decoded.as_str());
        match arr {
            Ok(res) => Ok(Self(res)),
            Err(_err) => Err(bincode::error::DecodeError::InvalidCharEncoding([0; 4])),
        }
    }
}

// # server -> client
#[derive(Clone, Decode, Encode)]
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

const MAX_MAP_NAME_LEN: usize = 64;
/**
 * All information about the server
 * so that the client can prepare the game.
 * E.g. current map
 */
#[derive(Decode, Encode)]
pub struct MsgSvServerInfo {
    pub map: NetworkStr<MAX_MAP_NAME_LEN>,
    pub game_type: NetworkStr<32>,
}

#[derive(Decode, Encode)]
pub struct MsgSvPlayerInfo {
    pub info: MsgObjPlayerInfo,
}

#[derive(Decode, Encode)]
pub enum ServerToClientMessage {
    ServerInfo(MsgSvServerInfo),
    Snapshot(Snapshot),
    PlayerInfo(MsgSvPlayerInfo),
}

// # client message parts
#[derive(Clone, Decode, Encode)]
pub enum ColorChannel {
    R = 0,
    G,
    B,
    A,
}

#[derive(Clone, Decode, Encode)]
pub struct MsgObjGameSkinPartInfo {
    pub name: NetworkStr<{ 24 * 4 }>,
    pub color_swizzle_r: ColorChannel,
    pub color_swizzle_g: ColorChannel,
    pub color_swizzle_b: ColorChannel,
    pub color_swizzle_a: ColorChannel,
    pub color: vec4_base<u8>,
}

#[derive(Clone, Decode, Encode)]
pub struct MsgObjGameWeaponInfo {
    pub name: NetworkStr<{ 24 * 4 }>,
    pub anim_name: NetworkStr<{ 24 * 4 }>,
    pub effect_name: NetworkStr<{ 24 * 4 }>,
}

// # client -> server

#[derive(Decode, Encode, Copy, Clone, Default, Debug)]
pub enum WeaponType {
    // TODO
    #[default]
    TODO,
}

#[derive(Decode, Encode, Copy, Clone, Default, Debug)]
pub struct MsgObjPlayerInput {
    pub dir: i32,
    pub cursor_x: NetFloatIntegerRepType,
    pub cursor_y: NetFloatIntegerRepType,

    pub jump: bool,
    pub fire: i32,
    pub hook: bool,
    pub flags: i32,
    pub weapon_req: WeaponType,
    // TODO: next prev weapon, needed?
    pub weapon_diff: i32,
}

#[derive(Decode, Encode)]
pub struct MsgClReady {
    pub player_info: MsgObjPlayerInfo,
}

pub type MsgClInput = MsgObjPlayerInput;

#[derive(Decode, Encode)]
pub enum ClientToServerMessage {
    Ready(MsgClReady),
    Input(MsgClInput),
}

#[derive(Decode, Encode)]
pub enum GameMessage {
    ServerToClient(ServerToClientMessage),
    ClientToServer(ClientToServerMessage),
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
