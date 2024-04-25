use base::hash::Hash;
use ed25519_dalek::VerifyingKey;
use game_interface::types::{
    character_info::NetworkCharacterInfo,
    game::{GameEntityId, GameTickType},
    network_string::NetworkString,
};
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::player_input::PlayerInput;

use super::types::chat::NetChatMsg;

const MAX_MAP_NAME_LEN: usize = 64;
const MAX_GAME_TYPE_NAME_LEN: usize = 32;
/// All information about the server
/// so that the client can prepare the game.
/// E.g. current map
#[derive(Clone, Serialize, Deserialize)]
pub struct MsgSvServerInfo {
    /// the map that is currently played on
    pub map: NetworkString<MAX_MAP_NAME_LEN>,
    pub map_blake3_hash: Hash,
    /// - if this is `Some`, it is the port to the fallback resource download server.
    /// - if this is `None`, either resources are downloaded from a official resource
    ///     server or from a resource server stored in the server
    ///     browser information of this server.
    /// If both cases don't exist, no resources are downloaded, the client might stop connecting.
    /// Note: this is intentionally only a port. If the server contains a resource server in their
    /// server browser info, the client makes sure that the said server relates to this server
    /// (e.g. by a domain + subdomain DNS resolve check)
    pub resource_server_fallback: Option<u16>,
    /// as soon as the client has finished loading it might want to render something to the screen
    /// the server can give a hint what the best camera position is for that
    pub hint_start_camera_pos: vec2,
    /// the game type currently played
    pub game_type: NetworkString<MAX_GAME_TYPE_NAME_LEN>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MsgSvChatMsg {
    pub msg: NetChatMsg,
}

// # client -> server
#[derive(Serialize, Deserialize)]
pub struct MsgClReady {
    pub player_info: NetworkCharacterInfo,
    /// The client has the private key to this public key.
    /// This key is _only_ used to verify signatures
    /// to make sure a message comes from a specific client,
    /// which ultimately allows to use the public key
    /// to identify the user.
    /// It does not encrypt all messages, or prevent MITM attacks
    /// in a sense that the game server proxies all messages.
    pub public_key: Option<VerifyingKey>,
}

#[derive(Serialize, Deserialize)]
pub struct MsgClAddLocalPlayer {
    pub player_info: NetworkCharacterInfo,

    /// a hint for the server that this local player is a dummy
    pub as_dummy: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MsgClInput {
    pub inp: PlayerInput,

    pub diff_id: Option<u64>,

    pub for_monotonic_tick: GameTickType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MsgClChatMsg {
    Global {
        msg: NetworkString<256>,
    },
    GameTeam {
        team: u32, // TODO
        msg: NetworkString<256>,
    },
    Whisper {
        receiver_id: GameEntityId,
        msg: NetworkString<256>,
    },
}
/*
// # client -> server
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

*/
