use std::time::Duration;

use hiarc::Hiarc;
use math::math::vector::{dvec2, vec2};
use pool::{
    datatypes::{PoolLinkedHashMap, PoolString},
    rc::PoolRc,
};
use serde::{Deserialize, Serialize};
pub use strum::{EnumCount, EnumIter, IntoEnumIterator};

use crate::types::{
    character_info::{NetworkCharacterInfo, NetworkSkinInfo},
    emoticons::EmoticonType,
    game::{GameEntityId, GameTickType},
    weapons::WeaponType,
};

#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum CharacterBuff {
    /// the character has a ninja powerup (vanilla like ninja)
    Ninja,
    /// the character is in a ghost state
    /// for ddrace this is basically the /spec mode
    /// no hook or weapon is rendered
    Ghost,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CharacterBuffInfo {
    /// the remaining time, or `None` if unknown
    pub remaining_time: Option<Duration>,
}

#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum CharacterDebuff {
    /// character is freezed (e.g. ddrace freeze)
    Freeze,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CharacterDebuffInfo {
    /// the remaining time, or `None` if unknown
    pub remaining_time: Option<Duration>,
}

#[derive(
    Debug,
    Hiarc,
    Default,
    EnumIter,
    EnumCount,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
)]
pub enum TeeEye {
    #[default]
    Normal = 0,
    Angry,
    Pain,
    Happy,
    // TODO: needed? Dead,
    Surprised,
    Blink,
}

/// The ingame metric is 1 tile = 1.0 float units
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct CharacterRenderInfo {
    pub lerped_pos: vec2,
    pub lerped_vel: vec2,
    /// A value of `None` here means that the hook will not be rendered
    pub lerped_hook_pos: Option<vec2>,
    pub has_air_jump: bool,
    /// this is the last known cursor position of the server
    pub cursor_pos: dvec2,
    pub move_dir: i32,
    pub cur_weapon: WeaponType,
    /// How many ticks passed since the last attack recoil
    /// or `None` if the character never attacked yet
    pub recoil_ticks_passed: Option<GameTickType>,

    pub left_eye: TeeEye,
    pub right_eye: TeeEye,

    pub buffs: PoolLinkedHashMap<CharacterBuff, CharacterBuffInfo>,
    pub debuffs: PoolLinkedHashMap<CharacterDebuff, CharacterDebuffInfo>,

    /// How many animation ticks have passed for this character.
    /// This is used for synchronized map animations.
    /// If unsure which value to set this to, simply set it to the
    /// same value as `game_ticks_passed`, which is the common use case.
    pub animation_ticks_passed: GameTickType,
    /// How many game ticks have passed for this character.
    /// This is the race time, or ticks in an active round.
    pub game_ticks_passed: GameTickType,
    /// If the game has a game round countdown for this character,
    /// this should be set to `Some(cooldown)`.
    /// Else it should be set to `None`.
    /// This is usually a round timer e.g. for competitive games.
    pub game_round_ticks: Option<GameTickType>,

    /// emoticon ticks passed & emoticon type
    pub emoticon: Option<(GameTickType, EmoticonType)>,
}

/// The camera mode of the local player
#[derive(Debug, Default, Clone, Copy, Hiarc, Serialize, Deserialize)]
pub enum PlayerCameraMode {
    /// Follows the own character
    #[default]
    Default,
    /// Free camera, the user can look around in the map
    /// as wanted.
    Free,
    /// The camera is currently locked to a specific
    /// position in a map (e.g. a kill cam).
    LockedTo(vec2),
}

/// Information about the the player of a character.
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct CharacterPlayerInfo {
    /// What camera mode the player currently uses
    pub cam_mode: PlayerCameraMode,
}

/// General information about the character
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub info: PoolRc<NetworkCharacterInfo>,

    /// Since overloading the skin color is such a common thing (sided pvp),
    /// this skin info should be preferred over the one in [`CharacterInfo::info`]
    /// for rendering. The one in [`CharacterInfo::info`] can instead be the
    /// original requested one.
    pub skin_info: NetworkSkinInfo,

    /// The stage in which the character currently is
    pub stage_id: Option<GameEntityId>,

    /// Does a player own this character.
    /// `None` for server side dummies or similar.
    pub player_info: Option<CharacterPlayerInfo>,

    /// The score that should be displayed in the server browser.
    /// Can e.g. also be a finish time.
    pub browser_score: PoolString,
    /// Which Tee eyes to show in the browser (e.g. for afk Tees).
    pub browser_eye: TeeEye,
}

/// information about the local character
#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
pub struct LocalCharacterRenderInfo {
    pub health: u32,
    pub armor: u32,

    /// A value of `None` means unlimited
    pub ammo_of_weapon: Option<u32>,
}
