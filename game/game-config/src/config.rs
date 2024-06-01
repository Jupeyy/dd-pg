use config::config::ConfigEngine;
use config_macro::{config_default, ConfigInterface};
use serde::{Deserialize, Serialize};

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigClient {
    #[default = 0]
    pub refresh_rate: u64,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigPlayerSkin {
    #[conf_valid(length(max = 24))]
    #[default = "default"]
    pub name: String,
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
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigServerAccounts {
    /// A non empty email activates accounts.
    #[default = ""]
    pub email: String,
    #[default = ""]
    pub password: String,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
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
    #[default = Default::default()]
    pub accounts: ConfigServerAccounts,
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
    /// empty string, "default", "vanilla" & "ddnet"
    /// are reserved names and will not cause
    /// loading a game mod module
    #[default = ""]
    pub game_mod: String,
    /// The game type is the game mode that should be played.
    /// This can be arbitrary and is just a hint for the game mod.
    /// E.g. "dm" or "ctf"
    #[default = ""]
    pub game_type: String,
    #[default = Default::default()]
    /// The database configuration.
    /// They should be used if the mod requires database support.
    /// Databases are generally optional.
    pub db: ConfigServerDatabase,
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
