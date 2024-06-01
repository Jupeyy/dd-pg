use std::collections::HashMap;

use crate::{self as config};

use anyhow::anyhow;
use atomic_enum::atomic_enum;
use config_macro::{config_default, ConfigInterface};
use hiarc::Hiarc;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

#[atomic_enum]
#[repr(u8)]
#[derive(Hiarc, Default, PartialEq, FromPrimitive, Serialize, Deserialize, ConfigInterface)]
pub enum GFXDebugModes {
    #[default]
    None = 0,
    Minimum,
    AffectsPerformance,
    Verbose,
    All,
}

pub type Query = HashMap<String, String>;

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

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigInput {
    /// make the mouse not grab
    #[default = false]
    pub dbg_mode: bool,
}

#[config_default]
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigGFX {
    #[default = 800]
    pub window_width: u32,
    #[default = 600]
    pub window_height: u32,
    #[default = 0]
    pub window_fullscreen_mode: u32,
    #[default = "Vulkan"]
    pub backend: String,
}

#[config_default]
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigSound {
    #[default = "kira"]
    pub backend: String,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigUi {
    // the ui path represents a path or set of action that
    // was clicked in the ui to get to the current position
    // it should be used similar to a URL and has URI syntax
    pub path: ConfigPath,
    /// a storage, similar to a local storage in browser
    pub storage: HashMap<String, String>,
    #[default = "autumn_day"]
    pub menu_background_map: String,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigNetwork {
    #[default = std::time::Duration::from_secs(20)]
    pub timeout: std::time::Duration,
    #[default = false]
    pub disable_retry_on_connect: bool,
}

#[config_default]
#[derive(Debug, Hiarc, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigDebug {
    pub gfx: GFXDebugModes,
    // show various "benchmarks" (e.g. loading of components etc.)
    #[default = false]
    pub bench: bool,
    #[default = false]
    pub untrusted_cert: bool,
}

#[config_default]
#[derive(Debug, Hiarc, Serialize, Deserialize, ConfigInterface, Clone)]
pub struct ConfigBackend {
    #[default = -0.5]
    pub global_texture_lod_bias: f64,
    #[default = 0]
    pub thread_count: u32,
    #[default = 0]
    pub msaa_samples: u32,
    #[default = false]
    pub vsync: bool,
}

#[config_default]
#[derive(Debug, Serialize, Deserialize, ConfigInterface)]
pub struct ConfigEngine {
    // input
    pub inp: ConfigInput,
    // sound

    // ui
    pub ui: ConfigUi,
    // graphics
    pub gfx: ConfigGFX,
    // sound
    pub snd: ConfigSound,
    // network
    pub net: ConfigNetwork,
    // debug
    pub dbg: ConfigDebug,
    // backend / graphics library
    pub gl: ConfigBackend,
}

impl ConfigEngine {
    pub fn new() -> ConfigEngine {
        ConfigEngine {
            inp: ConfigInput::default(),
            ui: ConfigUi::default(),
            gfx: ConfigGFX::default(),
            snd: ConfigSound::default(),
            net: ConfigNetwork::default(),
            dbg: ConfigDebug::default(),
            gl: ConfigBackend::default(),
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

#[cfg(test)]
mod test {
    use crate::{self as config};
    use config_macro::{config_default, ConfigInterface};
    use serde::{Deserialize, Serialize};

    #[test]
    fn it_works() {
        #[config_default]
        #[derive(Debug, Serialize, Deserialize, ConfigInterface)]
        pub struct ConfigTest {
            #[conf_valid(range(min = -2.0, max = 125.0))]
            #[default = -0.5]
            pub some_float: f64,
            #[conf_valid(range(min = 2, max = 125))]
            #[default = 123]
            pub some_u32: u32,
            #[default = true]
            pub some_bool: bool,
            #[conf_valid(length(min = 2, max = 10))]
            #[default = "hi test"]
            pub some_str: String,
            #[conf_valid(length(min = 2, max = 10))]
            #[default = vec![234, 567, 890]]
            pub some_vec: Vec<i32>,
        }

        let res = ConfigTest::default();
        assert!(res.some_bool == true);
        assert!(res.some_float == -0.5);
        assert!(res.some_u32 == 123);
        assert!(res.some_str == "hi test");
        assert!(res.some_vec == vec![234, 567, 890]);

        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();

        // nothing was changed
        assert!(res.some_bool == true);
        assert!(res.some_float == -0.5);
        assert!(res.some_u32 == 123);
        assert!(res.some_str == "hi test");
        assert!(res.some_vec == vec![234, 567, 890]);

        res.some_str = "".into();
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // because the min length is 2 it is filled with the default value
        assert!(res.some_str == "hi");

        res.some_str = "hi test very long, in fact too long".into();
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // because the max length is 10 it is truncated
        assert!(res.some_str == "hi test ve");

        res.some_str = "こんにちは、テスト".into();
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // should work, bcs length respects the unicode length not `s.len()`
        assert!(res.some_str == "こんにちは、テスト");

        res.some_str = "こんにちは、テスト、テスト".into();
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // unicode trunctation
        assert!(res.some_str == "こんにちは、テスト、");

        res.some_str = "こ".into();
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // fills i from default value
        assert!(res.some_str == "こi");

        res.some_vec = vec![123];
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // fills 567 from default
        assert!(res.some_vec == vec![123, 567]);

        res.some_vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // truncate
        assert!(res.some_vec == vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        res.some_bool = false;
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // random useless test
        assert!(res.some_bool == false);

        res.some_u32 = 126;
        res.some_float = 126.0;
        let serialized = serde_json::to_string(&res).unwrap();
        let mut res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // range check clamps value
        assert!(res.some_u32 == 125);
        assert!(res.some_float == 125.0);

        res.some_u32 = 0;
        res.some_float = -3.0;
        let serialized = serde_json::to_string(&res).unwrap();
        let res: ConfigTest = serde_json::from_str(&serialized).unwrap();
        // range check clamps value
        assert!(res.some_u32 == 2);
        assert!(res.some_float == -2.0);

        let res: ConfigTest = serde_json::from_str("{}").unwrap();

        // empty string should still fill values correctly
        assert!(res.some_bool == true);
        assert!(res.some_float == -0.5);
        assert!(res.some_u32 == 123);
        assert!(res.some_str == "hi test");
        assert!(res.some_vec == vec![234, 567, 890]);

        let res: ConfigTest = serde_json::from_str("{\"some_bool\": false}").unwrap();

        // partially filled string should still fill missing values correctly
        assert!(res.some_bool == false);
        assert!(res.some_float == -0.5);
        assert!(res.some_u32 == 123);
        assert!(res.some_str == "hi test");
        assert!(res.some_vec == vec![234, 567, 890]);
    }
}
