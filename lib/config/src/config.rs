use std::{collections::HashMap, time::Duration};

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
}

fn gfx_backend_default() -> String {
    "Vulkan".to_string()
}

fn cl_refresh_rate_default() -> u64 {
    0
}

fn sv_map_default() -> String {
    "ctf1".to_string()
}

fn sv_port_default() -> u16 {
    8310
}

fn sv_port_internal_default() -> u16 {
    0
}

fn sv_max_players_default() -> usize {
    64
}

fn sv_max_players_per_ip_default() -> usize {
    4
}

fn net_timeout_default() -> Duration {
    Duration::from_secs(20)
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    // client
    pub cl_background_show_tile_layers: bool,
    pub cl_overlay_entities: i32,

    #[serde(default = "cl_refresh_rate_default")]
    pub cl_refresh_rate: u64,
    // input
    pub inp_mousesens: u64,
    pub inp_mouse_follow_factor: u64,
    pub inp_mouse_deadzone: u64,
    pub inp_mouse_min_distance: u64,
    pub inp_mouse_max_distance: u64,

    // sound

    // ui
    // the ui path represents a path or set of action that
    // was clicked in the ui to get to the current position
    // it should be used similar to a URL and has URI syntax
    pub ui_path: ConfigPath,
    pub ui_last_server_addr: String,

    // graphics
    pub gfx_no_clip: bool,
    pub gfx_high_detail: bool,
    pub gfx_window_width: u32,
    pub gfx_window_height: u32,
    pub gfx_window_fullscreen_mode: u32,
    pub gfx_thread_count: usize,

    #[serde(default = "gfx_backend_default")]
    pub gfx_backend: String,
    // server
    #[serde(default = "sv_map_default")]
    pub sv_map: String,
    #[serde(default = "sv_port_default")]
    pub sv_port: u16,
    /// port for the internal server (inside the client)
    /// defaults to 0 -> random port
    #[serde(default = "sv_port_internal_default")]
    pub sv_port_internal: u16,
    #[serde(default = "sv_max_players_default")]
    pub sv_max_players: usize,
    #[serde(default = "sv_max_players_per_ip_default")]
    pub sv_max_players_per_ip: usize,

    // network
    #[serde(default = "net_timeout_default")]
    pub net_timeout: Duration,

    // debug
    pub dbg_gfx: EDebugGFXModes,
    // show various "benchmarks" (e.g. loading of components etc.)
    pub dbg_bench: bool,
    pub dbg_untrusted_cert: bool,
    // runtime hints, not saved
}

impl Config {
    pub fn new() -> Config {
        Config {
            cl_background_show_tile_layers: true,
            cl_overlay_entities: 0,
            cl_refresh_rate: cl_refresh_rate_default(),

            gfx_no_clip: false,
            gfx_high_detail: true,

            gfx_window_width: 800,
            gfx_window_height: 600,

            gfx_thread_count: 1,
            gfx_backend: gfx_backend_default(),

            sv_map: sv_map_default(),
            sv_port: sv_port_default(),
            sv_port_internal: sv_port_internal_default(),
            sv_max_players: sv_max_players_default(),
            sv_max_players_per_ip: sv_max_players_per_ip_default(),

            net_timeout: net_timeout_default(),
            ..Default::default()
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
