use config::{config::ConfigEngine, types::ConfRgb};
use config::{config_default, ConfigInterface};
use game_interface::types::character_info::NetworkSkinInfo;
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
pub struct ConfigClient {
    #[default = 0]
    pub refresh_rate: u64,
    /// Dummy related settings
    #[default = Default::default()]
    pub dummy: ConfigDummy,
    /// Show nameplates
    #[default = true]
    pub nameplates: bool,
    /// Show nameplate of the own character
    #[default = false]
    pub own_nameplate: bool,
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
    pub skin: ConfigPlayerSkin,
    #[conf_valid(length(max = 3))]
    #[default = "ENG"]
    pub country: String,
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
pub struct ConfigServerDatabase {
    /// A non empty username activates the database.
    #[default = ""]
    pub username: String,
    #[default = ""]
    pub password: String,
    #[default = ""]
    pub database: String,
    #[default = "127.0.0.1"]
    pub host: String,
    #[default = 3306]
    pub port: u16,
    #[default = ""]
    pub ca_cert_path: String,
    #[default = false]
    pub enable_accounts: bool,
    #[default = 64]
    pub connection_count: u64,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigServer {
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
}

#[config_default]
#[derive(Debug, Clone, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigSound {
    /// Use spartial emitters for ingame sounds.
    #[default = false]
    pub spartial: bool,
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
}
