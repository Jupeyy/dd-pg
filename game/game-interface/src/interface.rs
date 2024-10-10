use std::{num::NonZeroU64, sync::Arc, time::Duration};

use base::hash::Hash;
pub use base_io::io_batcher::IoBatcher;
pub use command_parser::parser::CommandArg;
use ddnet_accounts_types::account_id::AccountId;
use game_database::traits::{DbInterface, DbKind};
use hiarc::Hiarc;
use math::math::vector::vec2;
use pool::{datatypes::PoolLinkedHashMap, mt_datatypes::PoolCow as MtPoolCow};
use serde::{Deserialize, Serialize};

use crate::{
    chat_commands::ChatCommands,
    client_commands::ClientCommand,
    events::{EventClientInfo, EventId, GameEvents},
    rcon_commands::RconCommands,
    types::{
        character_info::NetworkCharacterInfo,
        emoticons::EmoticonType,
        game::{GameEntityId, NonZeroGameTickType},
        input::CharacterInputInfo,
        network_stats::PlayerNetworkStats,
        network_string::{NetworkReducedAsciiString, NetworkString},
        player_info::{PlayerClientInfo, PlayerDropReason},
        render::{
            character::{CharacterInfo, LocalCharacterRenderInfo, TeeEye},
            scoreboard::Scoreboard,
            stage::StageRenderInfo,
        },
        snapshot::{FromSnapshotBuildMode, SnapshotClientInfo, SnapshotLocalPlayers},
        ticks::TickOptions,
    },
    vote_commands::VoteCommand,
};

/// Some options for creating the game
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct GameStateCreateOptions {
    /// the max number of characters is usually also used for
    /// the number of characters, the number of stages etc.
    pub hint_max_characters: Option<usize>,

    /// The mod specific config is loaded in a specific way:
    /// - <mod>.json is tried to be loaded
    ///
    /// The client never loads any config, the server can send config
    /// information over [`GameStateStaticInfo::config`].
    /// If `None`, then no config was found.
    pub config: Option<Vec<u8>>,

    /// Which kind of database holds the account information
    pub account_db: Option<DbKind>,
}

#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct GameStateServerOptions {
    /// This is the name of the physics group.
    /// This is mostly interesting for the client to select
    /// the right physics layer assets
    /// Examples of names are `vanilla`, `ddnet`.
    pub physics_group_name: NetworkReducedAsciiString<24>,
    /// Whether stages/ddrace-teams are allowed on this server.
    pub allow_stages: bool,
    /// Whether the client should show a "Pick a side"-button to
    /// switch between red & blue sides.
    pub use_vanilla_sides: bool,
    /// Whether the game server uses accounts where the ingame name
    /// and the account name are split and the client should show
    /// an extra UI tab for changing the account name and display
    /// standard account information.
    ///
    /// See also [`crate::account_info::AccountInfo`].
    pub use_account_name: bool,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct GameStateStaticInfo {
    /// How many ticks should there be in a second.
    /// Also known as ticks per second
    pub ticks_in_a_second: NonZeroGameTickType,

    /// Chat commands supported by the mod
    pub chat_commands: ChatCommands,
    /// Rcon commands supported by the mod
    pub rcon_commands: RconCommands,

    /// A config file for this mod.
    /// On a server this config is sent to all clients,
    /// and also saved to disk.
    /// On the client this field is usually ignored.
    /// If no config is needed or no default config
    /// should be written to disk, leave this `None`.
    pub config: Option<Vec<u8>>,

    /// The name of the mod (this name is usually used inside the server browser/info)
    pub mod_name: NetworkString<24>,
    /// A version of this mod as string that is shown in server browser.
    /// It is allowed to be left empty
    pub version: String,

    /// Some options for the client (send by server)
    pub options: GameStateServerOptions,
}

/// Describes an interface to create a new game-state
pub trait GameStateCreate {
    /// `map` is intentionally left as arbitrary bytes.
    /// If the loaded mod supports custom maps, it can parse
    /// it however it wants.
    /// `io_batcher` helps to handle async tasks in sync context
    /// `db` gives access to the database, implementations generally should assume
    /// that database logic fails (for example because a dummy database is used)
    fn new(
        map: Vec<u8>,
        map_name: String,
        options: GameStateCreateOptions,
        io_batcher: IoBatcher,
        db: Arc<dyn DbInterface>,
    ) -> (Self, GameStateStaticInfo)
    where
        Self: Sized;
}

/// This is the interface for the client & server to communicate with the
/// core game component.
///
/// The core game component is basically how the game changes on ticks, when
/// input comes in, players are joining etc.
/// The 3 main areas in the interface are:
///
/// - handle game, player input, players joining/leaving, generating snapshots
///     (which is usually called by both server & client)
/// - collecting render information, which the client uses to render all game objects
/// - handling of certain events, e.g. how chat is displayed in the client
///     or how chat commands are processed, rcon handling etc.
///
/// # Prediction code
/// Maybe one of the harder parts is that prediction happens inside the physics module,
/// here are some tips to help making the implementation work as intended.
///
/// Generally the prediction assumes that there are two worlds:
/// - A previous world state (usually filled by [`GameStateInterface::build_from_snapshot_for_prev`])
/// - The current world state (which for anti-ping should be up to date with the world on the server)
///
/// If you use id generators for entities and you update them based on snapshots,
/// never update id generators for the "normal" world by a snapshot intended for prediction (previous) worlds.
///
/// The previous world state is always set by client/server,
/// for example by using [`GameStateInterface::build_from_snapshot_for_prev`].
///
/// For anti-ping predicted worlds, the implementation works almost like the server.
/// For non-anti-ping predicted worlds the client/server [`GameStateInterface::build_from_snapshot`] with appropriate
/// options.
///
/// Understanding future tick prediction:
/// The client can additionally do a different kind of prediction, where the latest known input of the local players
/// are used to predict what _could_ happen if there wouldn't be any input delay.  
/// Hints for this kind of prediction are usually tagged as `is_future_tick_prediction`.  
/// If this prediction mode is `true` the implementation should disable code like spawning
/// entities, killing entities and other stuff which might be easily miss predicted otherwise.
///
/// #### Teleportation & demo friendly snapshots
///
/// If your snapshot (respectively game entities) has a counter for non-linear (whatever) events,
/// so that e.g. a teleport of an entity position would increase this counter.
/// (Whatever your imagination can add here to make demo playback logically and smooth at the same time).
/// Then this function can use the old characters position instead of the new one,
/// so that there is no interpolation between these position, which is quite likely to happen if you
/// play back a demo in slow motion.
pub trait GameStateInterface: GameStateCreate {
    /// A player loaded the map (and whatever) and is ready to join the game.
    /// This function returns an entity id, which the server/client use to identify
    /// the player for snapshots or similar things.
    fn player_join(&mut self, player_info: &PlayerClientInfo) -> GameEntityId;
    /// The player disconnected from the game. The client/server will not associate
    /// anything locally with that id anymore
    fn player_drop(&mut self, player_id: &GameEntityId, reason: PlayerDropReason);

    /// Set the input of one or more players:
    fn set_player_inputs(&mut self, inps: PoolLinkedHashMap<GameEntityId, CharacterInputInfo>);

    /// The player tried to emote.
    fn set_player_emoticon(&mut self, player_id: &GameEntityId, emoticon: EmoticonType);

    /// Change the tee's eyes for a certain amount of time.
    /// If the mod should not support this, simply ignore this event.
    fn set_player_eye(&mut self, player_id: &GameEntityId, eye: TeeEye, duration: Duration);

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

    /// A notification event that a new account was created.
    /// The mod could rewrite database entries that previously used
    /// the public key information (see [`crate::types::player_info::PlayerUniqueId`]),
    /// and link them to the account id instead.
    fn account_created(&mut self, account_id: AccountId, cert_fingerprint: Hash);

    /// Network stats for all known players
    /// This is usually only called on the server.
    /// Normally this should be included in snapshots to
    /// render the ping and network health in the scoreboard.
    /// It should not be expected that this is called more than once per second.
    fn network_stats(&mut self, stats: PoolLinkedHashMap<GameEntityId, PlayerNetworkStats>);

    /// A client command initiated by a user (e.g. killing, switching to spectators etc.)
    fn client_command(&mut self, player_id: &GameEntityId, cmd: ClientCommand);

    /// The result of a vote that the game implementation should be aware of.
    fn vote_command(&mut self, cmd: VoteCommand);

    // stuff that is rendered
    /// Collects scoreboard information, see [`Scoreboard`]
    fn collect_scoreboard_info(&self) -> Scoreboard;
    /// Get the render info for all stages of interest.
    fn all_stages(&self, ratio: f64) -> PoolLinkedHashMap<GameEntityId, StageRenderInfo>;
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
    fn tick(&mut self, options: TickOptions);

    // snapshot related
    /// Builds an opaque snapshot out of the current game state.
    /// This opaque snapshot must be restorable by [`GameStateInterface::build_from_snapshot`],
    /// thus it usually contains all information required to build the
    /// game state from pre-existing state.
    #[must_use]
    fn snapshot_for(&self, client: SnapshotClientInfo) -> MtPoolCow<'static, [u8]>;

    /// Builds the game state out of an opaque snapshot previously build by [`GameStateInterface::snapshot_for`].
    /// This tick can be from the past and from the future, so a snapshot should generally be able to overwrite
    /// the full game state.
    /// Returns a list of local players (which is usually only interesting for the client).
    #[must_use]
    fn build_from_snapshot(
        &mut self,
        snapshot: &MtPoolCow<'static, [u8]>,
        mode: FromSnapshotBuildMode,
    ) -> SnapshotLocalPlayers;

    /// Builds an opaque snapshot out of the current game state, but for server side only.
    /// Normally this can share most code with [`GameStateInterface::snapshot_for`]
    /// Implementing it is optional.
    #[must_use]
    fn snapshot_for_hotreload(&self) -> Option<MtPoolCow<'static, [u8]>>;

    /// Builds the game state out of an opaque snapshot previously build by [`GameStateInterface::snapshot_for_hotreload`].
    /// It's generally encouraged that the mod can deal with errors, e.g. if the binary interface changed.
    fn build_from_snapshot_by_hotreload(&mut self, snapshot: &MtPoolCow<'static, [u8]>);

    /// Builds the game state out of an opaque snapshot previously build by [`GameStateInterface::snapshot_for`].
    /// The difference to [`GameStateInterface::build_from_snapshot`] is that this function is intended to be used
    /// for the previous game state, which is ultimately used for prediction.
    ///
    /// This is useful for client components like a demo player.
    fn build_from_snapshot_for_prev(&mut self, snapshot: &MtPoolCow<'static, [u8]>);

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
