use std::{collections::HashMap, time::Duration};

use anyhow::anyhow;
use config_macro::ConfigInterface;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[repr(u8)]
#[derive(
    Default, Clone, Copy, PartialEq, FromPrimitive, Serialize, Deserialize, ConfigInterface,
)]
pub enum EDebugGFXModes {
    #[default]
    None = 0,
    Minimum,
    AffectsPerformance,
    Verbose,
    All,
}

pub type Query = HashMap<String, String>;

impl crate::traits::ConfigInterface for Query {
    fn conf_value() -> crate::traits::ConfigValue {
        crate::traits::ConfigValue::JSONRecord {
            val_ty: Box::new(crate::traits::ConfigValue::Array {
                val_ty: Box::new(String::conf_value()),
            }),
        }
    }

    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
        if path.is_empty() {
            *self = Self::deserialize(serde_json::to_value(val)?)?;
            Ok(())
        } else {
            Err(anyhow!("Expected end of path, but found {path}"))
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigPath {
    pub name: String,
    pub query: Query,
}

impl ConfigPath {
    pub fn route(&mut self, full_path: &str) {
        self.name = full_path.to_string();
    }
    pub fn route_queried(&mut self, full_path: &str, mut queries: Vec<(String, String)>) {
        self.name = full_path.to_string();
        queries.drain(..).for_each(|q| {
            self.query.insert(q.0, q.1);
        });
    }
    pub fn route_query_only_single(&mut self, query: (String, String)) {
        self.query.insert(query.0, query.1);
    }

    pub fn try_route(&mut self, mod_name: &str, path: &str) {
        if Self::is_route_correct(mod_name, path).is_ok() {
            self.name = (if mod_name.is_empty() {
                "".to_string()
            } else {
                mod_name.to_string() + "/"
            } + path);
        }
    }

    pub fn is_route_correct(mod_name: &str, path: &str) -> anyhow::Result<()> {
        if let Some(_) = mod_name.find(|c: char| !c.is_ascii_alphabetic()) {
            Err(anyhow!("Mod name must only contain ascii characters"))
        } else {
            if let Some(_) = path.find(|c: char| !c.is_ascii_alphabetic()) {
                Err(anyhow!("Path name must only contain ascii characters"))
            } else {
                Ok(())
            }
        }
    }
}

#[derive(Serialize, Deserialize, ConfigInterface)]
pub struct ConfigClient {
    #[serde(default)]
    pub refresh_rate: u64,
}

impl Default for ConfigClient {
    fn default() -> Self {
        Self { refresh_rate: 0 }
    }
}

#[derive(Serialize, Deserialize, Validate, ConfigInterface)]
pub struct ConfigMap {
    #[serde(default)]
    pub high_detail: bool,
    #[serde(default)]
    pub background_show_tile_layers: bool,
    #[serde(default)]
    pub show_quads: bool,
    #[validate(range(min = 0, max = 100))]
    #[serde(default)]
    pub physics_layer_opacity: i32,
    #[serde(default)]
    pub text_entities: bool,
}

impl Default for ConfigMap {
    fn default() -> Self {
        Self {
            high_detail: true,
            background_show_tile_layers: true,
            show_quads: true,
            physics_layer_opacity: 0,
            text_entities: true,
        }
    }
}

#[derive(Serialize, Deserialize, ConfigInterface)]
pub struct ConfigInput {
    #[serde(default)]
    pub mouse_sensitivity: u64,
    #[serde(default)]
    pub mouse_follow_factor: u64,
    #[serde(default)]
    pub mouse_deadzone: u64,
    #[serde(default)]
    pub mouse_min_distance: u64,
    #[serde(default)]
    pub mouse_max_distance: u64,

    /// make the mouse not grab
    #[serde(default)]
    pub dbg_mode: bool,
}

impl Default for ConfigInput {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 100,
            mouse_follow_factor: 0,
            mouse_deadzone: 0,
            mouse_min_distance: 0,
            mouse_max_distance: 400,
            dbg_mode: false,
        }
    }
}

#[derive(Serialize, Deserialize, ConfigInterface)]
pub struct ConfigGFX {
    #[serde(default)]
    pub window_width: u32,
    #[serde(default)]
    pub window_height: u32,
    #[serde(default)]
    pub window_fullscreen_mode: u32,
    #[serde(default)]
    pub backend: String,
}

impl Default for ConfigGFX {
    fn default() -> Self {
        Self {
            window_width: 800,
            window_height: 600,
            window_fullscreen_mode: 0,
            backend: "Vulkan".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, ConfigInterface)]
pub struct ConfigServer {
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub port: u16,
    /// port for the internal server (inside the client)
    /// defaults to 0 -> random port
    #[serde(default)]
    pub port_internal: u16,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub max_players_per_ip: u32,
}

impl Default for ConfigServer {
    fn default() -> Self {
        Self {
            map: "ctf1".to_string(),
            port: 8310,
            port_internal: 0,
            max_players: 64,
            max_players_per_ip: 4,
        }
    }
}

#[derive(Serialize, Deserialize, ConfigInterface)]
pub struct ConfigUI {
    // the ui path represents a path or set of action that
    // was clicked in the ui to get to the current position
    // it should be used similar to a URL and has URI syntax
    #[serde(default)]
    pub path: ConfigPath,
    #[serde(default)]
    pub last_server_addr: String,
    #[serde(default)]
    pub menu_background_map: String,
}

impl Default for ConfigUI {
    fn default() -> Self {
        Self {
            path: ConfigPath::default(),
            last_server_addr: "".to_string(),
            menu_background_map: "autumn_day".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, ConfigInterface)]
pub struct ConfigNetwork {
    #[serde(default)]
    pub timeout: Duration,
}

impl Default for ConfigNetwork {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
        }
    }
}

#[derive(Default, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigDebug {
    pub gfx: EDebugGFXModes,
    // show various "benchmarks" (e.g. loading of components etc.)
    pub bench: bool,
    pub untrusted_cert: bool,
}

impl ConfigDebug {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

#[derive(Default, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigBackend {
    pub global_texture_lod_bias: f64,
    pub thread_count: u32,
    pub fsaa_samples: u32,
}

impl ConfigBackend {
    pub fn new() -> Self {
        Self {
            global_texture_lod_bias: -0.5,
            ..Default::default()
        }
    }
}

#[derive(Default, Serialize, Deserialize, ConfigInterface)]
pub struct Config {
    // client
    pub cl: ConfigClient,
    // map
    pub map: ConfigMap,
    // input
    pub inp: ConfigInput,
    // sound

    // ui
    pub ui: ConfigUI,
    // graphics
    pub gfx: ConfigGFX,
    // server
    pub sv: ConfigServer,
    // network
    pub net: ConfigNetwork,
    // debug
    pub dbg: ConfigDebug,
    // backend / graphics library
    pub gl: ConfigBackend,
    // runtime hints, not saved
}

impl Config {
    pub fn new() -> Config {
        Config {
            cl: ConfigClient::default(),
            map: ConfigMap::default(),
            inp: ConfigInput::default(),
            ui: ConfigUI::default(),
            gfx: ConfigGFX::default(),
            sv: ConfigServer::default(),
            net: ConfigNetwork::default(),
            dbg: ConfigDebug::new(),
            gl: ConfigBackend::new(),
        }
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
