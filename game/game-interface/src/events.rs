use std::time::Duration;

use bitflags::bitflags;
use hiarc::Hiarc;
use math::math::vector::vec2;
use pool::{
    datatypes::PoolLinkedHashSet,
    mt_datatypes::{PoolLinkedHashMap, PoolString, PoolVec},
};
use serde::{Deserialize, Serialize};

use crate::types::{
    flag::FlagType,
    game::GameEntityId,
    id_gen::{IdGenerator, IdGeneratorIdType},
    weapons::WeaponType,
};

/// The id of an event
pub type EventId = IdGeneratorIdType;

pub type EventIdGenerator = IdGenerator;

/// Sounds that a ninja spawns
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameBuffNinjaEventSound {
    /// a pickup spawned
    Spawn,
    /// a pickup was collected by a character
    Collect,
    /// user used attack
    Attack,
    /// hits an object/character
    Hit,
}

/// Effects that a ninja spawns
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameBuffNinjaEventEffect {}

/// Events caused by ninja
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameBuffNinjaEvent {
    /// See [GameBuffNinjaEventSound]
    Sound(GameBuffNinjaEventSound),
    /// See [GameBuffNinjaEventEffect]
    Effect(GameBuffNinjaEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameBuffEvent {
    Ninja(GameBuffNinjaEvent),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameDebuffFrozenEventSound {
    /// user (tried to) used attack
    Attack,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameDebuffFrozenEventEffect {}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameDebuffFrozenEvent {
    Sound(GameDebuffFrozenEventSound),
    Effect(GameDebuffFrozenEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameDebuffEvent {
    Frozen(GameDebuffFrozenEvent),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameCharacterEventSound {
    WeaponSwitch { new_weapon: WeaponType },
    NoAmmo { weapon: WeaponType },
    HammerFire,
    GunFire,
    GrenadeFire,
    LaserFire,
    ShotgunFire,
    GroundJump,
    AirJump,
    HookHitPlayer,
    HookHitHookable,
    HookHitUnhookable,
    Spawn,
    Death,
    Pain { long: bool },
    Hit { strong: bool },
    HammerHit,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameCharacterEventEffect {
    Spawn,
    Death,
    AirJump,
    DamageIndicator { vel: vec2 },
    HammerHit,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameCharacterEvent {
    Sound(GameCharacterEventSound),
    Effect(GameCharacterEventEffect),
    Buff(GameBuffEvent),
    Debuff(GameDebuffEvent),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameGrenadeEventSound {
    /// pickup spawned
    Spawn,
    /// a pickup was collected by a character
    Collect,
    Explosion,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameGrenadeEventEffect {
    Explosion,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameGrenadeEvent {
    Sound(GameGrenadeEventSound),
    Effect(GameGrenadeEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameLaserEventSound {
    /// pickup spawned
    Spawn,
    /// a pickup was collected by a character
    Collect,
    Bounce,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameLaserEventEffect {}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameLaserEvent {
    Sound(GameLaserEventSound),
    Effect(GameLaserEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameShotgunEventSound {
    /// pickup spawned
    Spawn,
    /// a pickup was collected by a character
    Collect,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameShotgunEventEffect {}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameShotgunEvent {
    Sound(GameShotgunEventSound),
    Effect(GameShotgunEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameFlagEventSound {
    /// a flag was collected by a character
    Collect(FlagType),
    /// flag was captured
    Capture,
    /// flag was dropped
    Drop,
    /// flag returned to spawn point
    Return,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameFlagEventEffect {}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameFlagEvent {
    Sound(GameFlagEventSound),
    Effect(GameFlagEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupHeartEventSound {
    Spawn,
    /// a pickup was collected by a character
    Collect,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupHeartEventEffect {}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupHeartEvent {
    Sound(GamePickupHeartEventSound),
    Effect(GamePickupHeartEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupArmorEventSound {
    Spawn,
    /// a pickup was collected by a character
    Collect,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupArmorEventEffect {}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupArmorEvent {
    Sound(GamePickupArmorEventSound),
    Effect(GamePickupArmorEventEffect),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GamePickupEvent {
    Heart(GamePickupHeartEvent),
    Armor(GamePickupArmorEvent),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameWorldEntityEvent {
    Character {
        /// A value of `None` here means that
        /// the event is a "global"/world event
        id: Option<GameEntityId>,
        ev: GameCharacterEvent,
    },
    Grenade {
        /// A value of `None` here means that
        /// the event is a "global"/world event
        id: Option<GameEntityId>,
        ev: GameGrenadeEvent,
    },
    Laser {
        /// A value of `None` here means that
        /// the event is a "global"/world event
        id: Option<GameEntityId>,
        ev: GameLaserEvent,
    },
    Shotgun {
        /// A value of `None` here means that
        /// the event is a "global"/world event
        id: Option<GameEntityId>,
        ev: GameShotgunEvent,
    },
    Flag {
        /// A value of `None` here means that
        /// the event is a "global"/world event
        id: Option<GameEntityId>,
        ev: GameFlagEvent,
    },
    Pickup {
        /// A value of `None` here means that
        /// the event is a "global"/world event
        id: Option<GameEntityId>,
        ev: GamePickupEvent,
    },
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct GameWorldPositionedEvent {
    /// 1 tile = 1 integer unit
    pub pos: vec2,
    pub ev: GameWorldEntityEvent,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameWorldSystemMessage {
    PlayerJoined { id: GameEntityId, name: PoolString },
    PlayerLeft { id: GameEntityId, name: PoolString },
    Custom(PoolString),
}

#[derive(
    Debug, Hiarc, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct KillFeedFlags(u32);
bitflags! {
    impl KillFeedFlags: u32 {
        /// killed by a wallshot, usually only interesting for laser
        const WALLSHOT = (1 << 0);
        /// the killer is dominating over the victims
        const DOMINATING = (1 << 1);
    }
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameWorldActionFeedKillWeapon {
    Weapon {
        weapon: WeaponType,
    },
    Ninja,
    /// Kill tiles or world border
    World,
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameWorldActionFeed {
    Kill {
        killer: Option<GameEntityId>,
        /// assists to the killer
        assists: PoolVec<GameEntityId>,
        victims: PoolVec<GameEntityId>,
        weapon: GameWorldActionFeedKillWeapon,
        flags: KillFeedFlags,
    },
    RaceFinish {
        characters: PoolVec<GameEntityId>,
        finish_time: Duration,
    },
    Custom(PoolString),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameWorldGlobalEvent {
    /// A system message
    System(GameWorldSystemMessage),
    /// A action feed item, kill message or finish time etc.
    ActionFeed(GameWorldActionFeed),
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum GameWorldEvent {
    Positioned(GameWorldPositionedEvent),
    Global(GameWorldGlobalEvent),
}

/// # ID (Event-ID)
/// All events have an ID, this ID is always unique across all worlds on the server for every single event.
/// The client tries to match a event by its ID, the client might reset the id generator tho, if
/// the server is out of sync.
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct GameWorldEvents {
    pub events: PoolLinkedHashMap<EventId, GameWorldEvent>,
}

pub type GameWorldsEvents = PoolLinkedHashMap<GameEntityId, GameWorldEvents>;

/// A collection of events that are interpretable by the client.
/// These events are automatically synchronized by the server with all clients.
///
/// # Important
/// Read the ID section of [`GameWorldEvents`]
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct GameEvents {
    pub worlds: GameWorldsEvents,

    /// the next id that would be peeked by
    /// the [`EventIdGenerator::peek_next_id`] function
    /// used to sync client & server ids
    pub event_id: EventId,
}

impl GameEvents {
    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }
}

/// When the server (or client) requests events it usually requests it for
/// certain players (from the view of these players).
/// Additionally it might want to opt-in into getting every event etc.
///
/// Generally the implementation is free to ignore any of these. This might
/// lead to inconsitencies in the user experience tho (see also [`crate::types::snapshot::SnapshotClientInfo`])
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct EventClientInfo {
    /// A list of players the client requests the snapshot for.
    /// Usually these are the local players (including the dummy).
    pub client_player_ids: PoolLinkedHashSet<GameEntityId>,
    /// A hint that everything should be snapped, regardless of the requested players
    pub everything: bool,
    /// A hint that all stages (a.k.a. ddrace teams) should be snapped
    /// (the client usually renders them with some transparency)
    pub other_stages: bool,
}
