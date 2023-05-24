use std::collections::HashMap;

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

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    // client
    pub cl_background_show_tile_layers: bool,
    pub cl_overlay_entities: i32,

    // sound

    // ui
    // the ui path represents a path or set of action that
    // was clicked in the ui to get to the current position
    // it should be used similar to a URL and has URI syntax
    pub ui_path: ConfigPath,

    // graphics
    pub gfx_no_clip: bool,
    pub gfx_high_detail: bool,
    pub gfx_window_width: u32,
    pub gfx_window_height: u32,
    pub gfx_window_fullscreen_mode: u32,
    pub gfx_thread_count: usize,
    // server

    // network

    // debug
    pub dbg_gfx: EDebugGFXModes,
    // show various "benchmarks" (e.g. loading of components etc.)
    pub dbg_bench: bool,
}

impl Config {
    pub fn new() -> Config {
        Config {
            gfx_no_clip: false,
            cl_background_show_tile_layers: true,
            cl_overlay_entities: 0,
            gfx_high_detail: true,

            gfx_window_width: 800,
            gfx_window_height: 600,

            gfx_thread_count: 1,
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

    pub fn save(&self) {
        let save_str = self.to_json_string();

        if let Ok(save_str) = save_str {
            std::fs::write("config.json", save_str).unwrap();
        }
    }

    pub fn load() -> Self {
        let res = std::fs::read("config.json");
        match res {
            Ok(file) => Self::from_json_string(String::from_utf8(file).unwrap().as_str())
                .unwrap_or(Config::new()),
            Err(_) => Self::new(),
        }
    }
}
