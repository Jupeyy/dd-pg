use std::collections::HashMap;

use config::config::ConfigPath;
use config::{config::ConfigEngine, types::ConfRgb};
use config::{config_default, ConfigInterface};
use game_interface::types::character_info::NetworkSkinInfo;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, ConfigInterface, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum ConfigDummyScreenAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigDummy {
    /// Show the dummy in a miniscreen
    #[default = false]
    pub mini_screen: bool,
    /// The percentual width of the miniscreens (per anchor)
    #[conf_valid(range(min = 1, max = 100))]
    #[default = 40]
    pub screen_width: u32,
    /// The percentual height of the miniscreens (per anchor)
    #[conf_valid(range(min = 1, max = 100))]
    #[default = 40]
    pub screen_height: u32,
    /// To where the mini screen is anchored.
    #[default = ConfigDummyScreenAnchor::TopRight]
    pub screen_anchor: ConfigDummyScreenAnchor,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigTeam {
    /// Sets a custom team name
    #[conf_valid(length(max = 24))]
    #[default = ""]
    pub name: String,
    /// The color of the team in the scoreboard
    #[default = Default::default()]
    pub color: ConfRgb,
}

#[config_default]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, ConfigInterface)]
pub struct NoiseFilterSettings {
    /// Attennuation in db
    #[default = 100.0]
    pub attenuation: f64,
    /// Threshold in db before processing is considered.
    #[default = -10.0]
    pub processing_threshold: f64,
}

#[config_default]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, ConfigInterface)]
pub struct ConfigSpatialChatNoiseGate {
    /// Threshold in db when to allow voice to come through the gate
    #[default = -36.0]
    pub open_threshold: f64,
    /// Threshold in db when to close the gate after previously playing voice data.
    #[default = -54.0]
    pub close_threshold: f64,
}

#[config_default]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, ConfigInterface)]
pub struct ConfigSpatialChatFilter {
    /// Whether to use a noise filter at all
    #[default = true]
    pub use_nf: bool,
    pub nf: NoiseFilterSettings,
    /// When to allow voice and when to close the gate
    /// when voice was previously played.
    pub noise_gate: ConfigSpatialChatNoiseGate,
    /// Microphone boost in db
    pub boost: f64,
}

#[config_default]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, ConfigInterface)]
pub struct ConfigSpatialChatPerPlayerOptions {
    /// Is the player muted completely?
    pub muted: bool,
    /// Whether to force a noise filter for this player.
    /// Note that this is generally a very expensive operation
    /// and uses lot of RAM.
    pub force_nf: bool,
    pub nf: NoiseFilterSettings,
    /// Whether to force a noise gate for
    /// this player. Uses extra CPU time.
    pub force_gate: bool,
    pub noise_gate: ConfigSpatialChatNoiseGate,
    /// Boost of the user's sound in db
    pub boost: f64,
}

#[config_default]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ConfigInterface)]
pub struct ConfigSpatialChat {
    /// Helper about if the user read about risks
    /// of using spatial chat
    #[default = false]
    pub read_warning: bool,
    /// Whether spatial chat is allowed (sending microphone data)
    #[default = false]
    pub activated: bool,
    /// Use spatial sound (instead of mono that gets more silent).
    #[default = true]
    pub spatial: bool,
    /// The sound driver
    pub host: String,
    /// The sound card
    pub device: String,
    /// Filter settings for the microphone
    pub filter: ConfigSpatialChatFilter,
    /// Allow to play voice from users that
    /// don't have an account.
    #[default = false]
    pub from_non_account_users: bool,
    /// Users with an account that are permanentally muted. The key
    /// is the account id as string
    pub account_players: HashMap<String, ConfigSpatialChatPerPlayerOptions>,
    /// Users withour an account that are permanentally muted.
    /// The key is the hash formatted as string
    pub account_certs: HashMap<String, ConfigSpatialChatPerPlayerOptions>,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigDemoRecorder {
    /// How many frames per second the video should have
    #[default = 60]
    pub fps: u32,
    /// How many pixels per logical unit there are.
    /// Higher values make UI elements bigger.
    #[default = 1.0]
    pub pixels_per_point: f64,
    /// The width of the video
    #[default = 1920]
    pub width: u32,
    /// The height of the video
    #[default = 1080]
    pub height: u32,
    /// Use hw accel
    #[default = ""]
    pub hw_accel: String,
    /// The sample rate for the audio stream.
    /// Should be a multiple of `fps` for best results.
    #[default = 48000]
    pub sample_rate: u32,
    /// "Constant Rate Factor" for x264.
    /// Where 0 is lossless and 51 is the worst.
    /// 18 is default.
    #[default = 18]
    pub crf: u8,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigClient {
    #[default = 0]
    pub refresh_rate: u64,
    /// Dummy related settings
    #[default = Default::default()]
    pub dummy: ConfigDummy,
    /// DDrace-Team related settings
    pub team: ConfigTeam,
    /// Show nameplates
    #[default = true]
    pub nameplates: bool,
    /// Show nameplate of the own character
    #[default = false]
    pub own_nameplate: bool,
    #[default = "autumn"]
    pub menu_background_map: String,
    /// Configs related to spatial chat support.
    pub spatial_chat: ConfigSpatialChat,
    /// Configurations for the demo video encoder.
    pub recorder: ConfigDemoRecorder,
    /// Apply input for prediction directly. Might cause miss prediction.
    pub instant_input: bool,
    /// Predict other entities that are not local as if the ping is 0.
    pub anti_ping: bool,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigPlayerSkin {
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub name: String,
    #[default = Default::default()]
    pub body_color: ConfRgb,
    #[default = Default::default()]
    pub feet_color: ConfRgb,
    /// Use the custom/user-defined colors for the skin
    #[default = false]
    pub custom_colors: bool,
}

impl From<&ConfigPlayerSkin> for NetworkSkinInfo {
    fn from(value: &ConfigPlayerSkin) -> Self {
        if value.custom_colors {
            Self::Custom {
                body_color: value.body_color.into(),
                feet_color: value.feet_color.into(),
            }
        } else {
            Self::Original
        }
    }
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigPlayer {
    #[conf_valid(length(max = 16))]
    #[default = "nameless tee"]
    pub name: String,
    #[conf_valid(length(max = 12))]
    #[default = ""]
    pub clan: String,
    #[default = Default::default()]
    pub skin: ConfigPlayerSkin,
    #[conf_valid(length(max = 7))]
    #[default = "default"]
    pub flag: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub weapon: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub freeze: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub ninja: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub game: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub ctf: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub hud: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub entities: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub emoticons: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub particles: String,
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub hook: String,
    #[default = Vec::new()]
    pub binds: Vec<String>,
}

impl ConfigPlayer {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigDummyProfile {
    /// An index for an array of [`ConfigPlayer`].
    #[default = 1]
    pub index: u64,
    /// Whether to copy assets from the main player's profile.
    #[default = true]
    pub copy_assets_from_main: bool,
    /// Whether to copy binds from the main player's profile.
    #[default = true]
    pub copy_binds_from_main: bool,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigPlayerProfiles {
    /// The main player. An index for an array of [`ConfigPlayer`].
    pub main: u64,
    /// The dummy of the main player.
    pub dummy: ConfigDummyProfile,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigMap {
    #[default = true]
    pub high_detail: bool,
    #[default = true]
    pub background_show_tile_layers: bool,
    #[default = true]
    pub show_quads: bool,
    #[conf_valid(range(min = 0, max = 100))]
    #[default = 0]
    pub physics_layer_opacity: u8,
    #[default = true]
    pub text_entities: bool,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigInput {
    #[default = 100]
    pub mouse_sensitivity: u64,
    #[default = 0]
    pub mouse_follow_factor: u64,
    #[default = 0]
    pub mouse_deadzone: u64,
    #[default = 0]
    pub mouse_min_distance: u64,
    #[default = 400]
    pub mouse_max_distance: u64,
}

#[config_default]
#[derive(Debug, Clone, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigServerDatabaseConnection {
    #[default = ""]
    pub username: String,
    #[default = ""]
    pub password: String,
    /// The database name.
    /// For sqlite this is the sqlite file
    #[default = ""]
    pub database: String,
    #[default = "127.0.0.1"]
    pub host: String,
    #[default = 3306]
    pub port: u16,
    /// Server certificate that the client trusts.
    /// Can be ignored for localhost & sqlite
    #[default = ""]
    pub ca_cert_path: String,
    #[default = 64]
    pub connection_count: u64,
}

#[config_default]
#[derive(Debug, Clone, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigServerDatabase {
    /// Connections to a database.
    /// The key value here is the type of databse (mysql, sqlite).
    /// Additionally the key allows `_backup` as suffix to connect to a backup database.
    pub connections: HashMap<String, ConfigServerDatabaseConnection>,
    /// Specify the database type where accounts will be enabled.
    /// Only one database type is allowed and must be enabled in the connections.
    #[default = ""]
    pub enable_accounts: String,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigServer {
    #[default = "unnamed server"]
    pub name: String,
    #[default = "ctf1"]
    pub map: String,
    #[default = 8310]
    pub port: u16,
    /// port for the internal server (inside the client)
    /// defaults to 0 -> random port
    #[default = 0]
    pub port_internal: u16,
    #[default = 64]
    pub max_players: u32,
    #[default = 4]
    pub max_players_per_ip: u32,
    #[default = false]
    pub register: bool,
    /// The game mod module to load
    /// empty string, "default", "native", "vanilla" & "ddnet"
    /// are reserved names and will not cause
    /// loading a game mod module
    #[default = ""]
    pub game_mod: String,
    #[default = Default::default()]
    /// The database configuration.
    /// They should be used if the mod requires database support.
    /// Databases are generally optional.
    pub db: ConfigServerDatabase,
    /// How many ticks must pass before sending the next snapshot
    #[conf_valid(range(min = 1, max = 100))]
    #[default = 2]
    pub ticks_per_snapshot: u64,
    /// Train a packet dictionary. (for compression)
    /// Don't activate this if you don't know what this means
    #[default = false]
    pub train_packet_dictionary: bool,
    #[conf_valid(range(min = 256, max = 104857600))]
    #[default = 65536]
    pub train_packet_dictionary_max_size: u32,
    /// Automatically make all maps that can be found
    /// as map vote. (Usually only recommended for test servers
    /// and local servers).
    #[default = false]
    pub auto_map_votes: bool,
    /// Whether to allow spatial chat on this server.
    /// Note that spatial chat causes lot of network
    /// traffic.
    #[default = false]
    pub spatial_chat: bool,
}

#[config_default]
#[derive(Debug, Clone, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigSound {
    /// Use spartial emitters for ingame sounds.
    #[default = false]
    pub spartial: bool,
    /// The sound volume for ingame sounds
    #[conf_valid(range(min = 0.0, max = 1.0))]
    #[default = 1.0]
    pub ingame_sound_volume: f64,
    /// The sound volume for map music/sounds
    #[conf_valid(range(min = 0.0, max = 1.0))]
    #[default = 1.0]
    pub map_sound_volume: f64,
    /// The overall volume multiplier
    #[conf_valid(range(min = 0.0, max = 1.0))]
    #[default = 0.3]
    pub global_volume: f64,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigDebugClientServerSyncLog {
    /// only works without ping jitter
    #[default = false]
    pub time: bool,
    #[default = false]
    pub inputs: bool,
    #[default = false]
    pub players: bool,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigDebug {
    /// log some sync related stuff from the internal server & client
    /// only use in release mode
    pub client_server_sync_log: ConfigDebugClientServerSyncLog,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigGame {
    // client
    pub cl: ConfigClient,
    // players
    #[conf_valid(length(min = 2))]
    #[default = vec![ConfigPlayer::default(), ConfigPlayer::new("brainless tee")]]
    #[conf_alias(player, players[0])]
    #[conf_alias(dummy, players[1])]
    pub players: Vec<ConfigPlayer>,
    pub profiles: ConfigPlayerProfiles,
    // map
    pub map: ConfigMap,
    // input
    pub inp: ConfigInput,
    // server
    pub sv: ConfigServer,
    // sound
    pub snd: ConfigSound,
    // debug for game
    pub dbg: ConfigDebug,
}

impl ConfigGame {
    pub fn new() -> ConfigGame {
        Self::default()
    }

    pub fn to_json_string(&self) -> anyhow::Result<String> {
        let res = serde_json::to_string_pretty(self)?;
        Ok(res)
    }

    pub fn from_json_string(json_str: &str) -> anyhow::Result<Self> {
        let res = serde_json::from_str(json_str)?;
        Ok(res)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub game: ConfigGame,
    pub engine: ConfigEngine,
}

impl Config {
    pub fn new(game: ConfigGame, engine: ConfigEngine) -> Config {
        Config { game, engine }
    }

    /// Shortcut for ui storage
    pub fn storage_opt<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.engine
            .ui
            .storage
            .get(key)
            .and_then(|str| serde_json::from_str(str).ok())
            .unwrap_or_default()
    }

    /// Shortcut for ui storage
    pub fn storage<T: Default + DeserializeOwned>(&self, key: &str) -> T {
        self.storage_opt(key).unwrap_or_default()
    }

    /// Shortcut for ui storage
    pub fn set_storage<T: Serialize>(&mut self, key: &str, data: &T) {
        self.engine
            .ui
            .storage
            .insert(key.to_string(), serde_json::to_string(&data).unwrap());
    }

    /// Shortcut for ui storage
    pub fn rem_storage(&mut self, key: &str) {
        self.engine.ui.storage.remove(key);
    }

    /// Shortcut for ui storage
    pub fn storage_entry(&mut self, key: &str) -> &mut String {
        self.engine.ui.storage.entry(key.to_string()).or_default()
    }

    /// Shortcut for ui path
    pub fn path(&mut self) -> &mut ConfigPath {
        &mut self.engine.ui.path
    }
}
