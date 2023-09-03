use std::{collections::HashMap, time::Duration};

use anyhow::anyhow;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Default, Clone, Copy, PartialEq, FromPrimitive, Serialize, Deserialize)]
pub enum EDebugGFXModes {
    #[default]
    None = 0,
    Minimum,
    AffectsPerformance,
    Verbose,
    All,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ConfigPath {
    pub name: String,
    pub query: HashMap<String, Vec<String>>,
}

impl ConfigPath {
    pub fn route(&mut self, full_path: &str) {
        self.name = full_path.to_string();
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

#[derive(Serialize, Deserialize)]
pub struct ConfigClient {
    #[serde(default)]
    pub background_show_tile_layers: bool,
    #[serde(default)]
    pub overlay_entities: i32,
    #[serde(default)]
    pub refresh_rate: u64,
}

impl Default for ConfigClient {
    fn default() -> Self {
        Self {
            background_show_tile_layers: true,
            overlay_entities: 0,
            refresh_rate: 0,
        }
    }
}

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct ConfigGFX {
    #[serde(default)]
    pub no_clip: bool,
    #[serde(default)]
    pub high_detail: bool,
    #[serde(default)]
    pub window_width: u32,
    #[serde(default)]
    pub window_height: u32,
    #[serde(default)]
    pub window_fullscreen_mode: u32,
    #[serde(default)]
    pub thread_count: usize,
    #[serde(default)]
    pub backend: String,
}

impl Default for ConfigGFX {
    fn default() -> Self {
        Self {
            no_clip: false,
            high_detail: true,

            window_width: 800,
            window_height: 600,
            window_fullscreen_mode: 0,
            thread_count: 1,
            backend: "Vulkan".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
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
    pub max_players: usize,
    #[serde(default)]
    pub max_players_per_ip: usize,
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

#[derive(Serialize, Deserialize)]
pub struct ConfigUI {
    // the ui path represents a path or set of action that
    // was clicked in the ui to get to the current position
    // it should be used similar to a URL and has URI syntax
    #[serde(default)]
    pub path: ConfigPath,
    #[serde(default)]
    pub last_server_addr: String,
}

impl Default for ConfigUI {
    fn default() -> Self {
        Self {
            path: ConfigPath::default(),
            last_server_addr: "".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
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

#[derive(Default, Serialize, Deserialize)]
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

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    // client
    pub cl: ConfigClient,
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
    // runtime hints, not saved
}

impl Config {
    pub fn new() -> Config {
        Config {
            cl: ConfigClient::default(),
            inp: ConfigInput::default(),
            ui: ConfigUI::default(),
            gfx: ConfigGFX::default(),
            sv: ConfigServer::default(),
            net: ConfigNetwork::default(),
            dbg: ConfigDebug::new(),
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
