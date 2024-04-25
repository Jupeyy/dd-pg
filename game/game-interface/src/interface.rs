use std::num::NonZeroU64;

use hiarc::Hiarc;
use math::math::vector::vec2;
use pool::{
    datatypes::{PoolLinkedHashMap, PoolVec},
    mt_datatypes::PoolVec as MtPoolVec,
};
use serde::{Deserialize, Serialize};

use crate::{
    client_commands::ClientCommand,
    events::{EventClientInfo, EventId, GameEvents},
    types::{
        character_info::NetworkCharacterInfo,
        game::{GameEntityId, GameTickType},
        input::{CharacterInput, CharacterInputConsumableDiff},
        player_info::PlayerClientInfo,
        render::{
            character::{CharacterInfo, CharacterRenderInfo, LocalCharacterRenderInfo},
            flag::FlagRenderInfo,
            laser::LaserRenderInfo,
            pickup::PickupRenderInfo,
            projectiles::ProjectileRenderInfo,
            scoreboard::ScoreboardGameType,
        },
        snapshot::{SnapshotClientInfo, SnapshotLocalPlayers},
    },
};

/// TODO: why is this here? Better way?
#[derive(Debug, Hiarc, Clone, Copy, Default, Serialize, Deserialize)]
pub enum GameType {
    #[default]
    Solo,
    Team,
}

/// Some options for creating the game
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct GameStateCreateOptions {
    /// the max number of characters is usually also used for
    /// the number of characters, the number of stages etc.
    pub hint_max_characters: Option<usize>,

    pub game_type: GameType,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct GameStateStaticInfo {
    /// How many ticks should there be in a second.
    /// Also known as ticks per second
    pub ticks_in_a_second: GameTickType,
    // TODO: supported chat commands + rcon
}

/// Describes an interface to create a new game-state
pub trait GameStateCreate {
    /// `map` is intentionally left as arbitrary bytes.
    /// If the loaded mod supports custom maps, it can parse
    /// it whoever it wants
    fn new(map: Vec<u8>, options: GameStateCreateOptions) -> (Self, GameStateStaticInfo)
    where
        Self: Sized;
}

/// This is the interface for the client & server to communicate with the
/// core game component.
/// The core game component is basically how the game changes on ticks, when
/// input comes in, players are joining etc.
/// The 3 main areas in the interface are:
/// - handle game, player input, players joining/leaving, generating snapshots
///     (which is usually called by both server & client)
/// - collecting render information, which the client uses to render all game objects
/// - handling of certain events, e.g. how chat is displayed in the client
///     or how chat commands are processed, rcon handling etc.
pub trait GameStateInterface: GameStateCreate {
    /// A player loaded the map (and whatever) and is ready to join the game.
    /// This function returns an entity id, which the server/client use to identify
    /// the player for snapshots or similar things.
    fn player_join(&mut self, player_info: &PlayerClientInfo) -> GameEntityId;
    /// The player disconnected from the game. The client/server will not associate
    /// anything locally with that id anymore
    fn player_drop(&mut self, player_id: &GameEntityId);

    /// Set the new player input:
    /// - the `inp` here is the current state of the input
    /// - the `diff` is the difference compared to the previous input,
    ///     which are the actions that happened compared to the previous input
    ///     (e.g. how often the player fired)
    fn set_player_input(
        &mut self,
        player_id: &GameEntityId,
        inp: &CharacterInput,
        diff: CharacterInputConsumableDiff,
    );

    /// A client changed its character info and notified the server about this change.
    /// Generally the implementation _can_ ignore the character info from the client
    /// and do whatever it wants. If it wants to conditionally apply and not apply
    /// this info, it should at least track the `version` field to prevent writing
    /// outdated information.
    ///
    /// # Versioning
    /// `version` is a strictly monotonic increasing version value. If the implementation
    /// receives an older version, that means the network packet arrived too late, it should
    /// be ignored.
    fn try_overwrite_player_character_info(
        &mut self,
        id: &GameEntityId,
        info: &NetworkCharacterInfo,
        version: NonZeroU64,
    );

    /// a kill that was initiated by the user (to respawn itself)
    fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand);

    // stuff that is rendered
    /// All projectiles that could potentially be rendered
    fn all_projectiles(&self, ratio: f64) -> PoolVec<ProjectileRenderInfo>;
    /// All flags that could potentially be rendered
    fn all_ctf_flags(&self, ratio: f64) -> PoolVec<FlagRenderInfo>;
    /// All lasers that could potentially be rendered
    fn all_lasers(&self, ratio: f64) -> PoolVec<LaserRenderInfo>;
    /// All pickups that could potentially be rendered
    fn all_pickups(&self, ratio: f64) -> PoolVec<PickupRenderInfo>;
    /// Contains all information about characters that should be rendered
    fn collect_characters_render_info(
        &self,
        intra_tick_ratio: f64,
    ) -> PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>;
    /// Collects scoreboard information, see [ScoreboardGameType]
    fn collect_scoreboard_info(&self) -> ScoreboardGameType;
    /// Collect information about the local character of a player
    fn collect_character_local_render_info(
        &self,
        player_id: &GameEntityId,
    ) -> LocalCharacterRenderInfo;

    /// Differently to [GameStateInterface::collect_characters_render_info] the result __must__
    /// contain information about all known characters. Even if not visible.
    /// This even includes spectators or server side dummies etc.
    /// This function is called by server & client
    fn collect_characters_info(&self) -> PoolLinkedHashMap<GameEntityId, CharacterInfo>;

    /// Retrieve a position the client should first see when connecting.
    /// If the client joins as spectator it could make sense to show the position
    /// where most action is happening for example.
    /// Or if the client joins the game directly it could be the most likely
    /// spawn position to prevent camera teleportations.
    fn get_client_camera_join_pos(&self) -> vec2;

    /// Advances the game state by one tick.
    fn tick(&mut self);

    /// Predict the next game state using the given character input.
    /// The input should be applied after syncing the prediction world
    /// with the non-prediction world.
    ///
    /// #### Small implementation hint
    /// If snapshots are implemented like requested in
    /// [GameStateInterface::snapshot_for],
    /// then a prediction tick is nothing different to
    /// taking & apply a snapshot on a pred world -> apply all input -> do a tick
    /// (Just make sure to not accidentially overwrite data
    /// that is not indentended to be written by a prediction tick
    /// like filling an event queue).
    fn pred_tick(
        &mut self,
        inps: PoolLinkedHashMap<GameEntityId, (CharacterInput, CharacterInputConsumableDiff)>,
    );

    // snapshot related
    /// Builds a opaque snapshot out of the current game state.
    /// This opaque snapshot can also be a snapshot delta, it's up to the
    /// implementation to handle this.
    #[must_use]
    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolVec<u8>;

    /// Writes a opaque snapshot previously build by [`GameStateInterface::snapshot_for`] into a game state.
    /// This tick can be from the past and from the future, so a snapshot should generally be able to overwrite
    /// the full game state.
    /// Returns a list of local players (which is usually only interesting for the client).
    #[must_use]
    fn build_from_snapshot(&mut self, snapshot: &MtPoolVec<u8>) -> SnapshotLocalPlayers;

    /// Builds game events that can be interpreted by the client.
    /// The server will call this function to sync it to the clients,
    /// the clients will call this to predict those events,
    /// it will try to not duplicate them by syncing it with the events
    /// send by the server.
    /// Other than snapshots, events are transparent. Additionally events
    /// are guaranteed to be sent in order and must only be sent exactly once.
    /// Events might be handled async to snapshots and other logic,
    /// the client can generally safely deal with invalid game ids etc.
    fn events_for(&self, client: EventClientInfo) -> GameEvents;

    /// A hint by the server/client that the implementation can now safely delete
    /// previously cached events.
    /// The idea behind this call is:
    /// - [`GameStateInterface::tick`] (or other functions) collect events
    /// - Server/client calls [`GameStateInterface::events_for`]
    ///     for every client that is of interest
    /// - Server/client calls this function so the implementation can clear all events
    fn clear_events(&mut self);

    /// set the event generator's id to this one
    fn sync_event_id(&self, event_id: EventId);
}
