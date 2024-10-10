use hiarc::Hiarc;
use math::math::vector::ubvec4;
use serde::{Deserialize, Serialize};

use crate::types::resource_key::NetworkResourceKey;

use super::network_string::NetworkString;

// # network part
#[derive(Debug, Hiarc, Default, Copy, Clone, Serialize, Deserialize)]
pub enum NetworkSkinInfo {
    #[default]
    Original,
    Custom {
        body_color: ubvec4,
        feet_color: ubvec4,
    },
}

#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct NetworkCharacterInfo {
    pub name: NetworkString<16>,
    pub clan: NetworkString<12>,
    /// Country has a max length of 7 characters
    /// ISO 3166-2 needs 6 characters.
    /// The word "default" 7.
    pub flag: NetworkString<7>,
    pub skin_info: NetworkSkinInfo,

    // resources
    pub skin: NetworkResourceKey<24>,
    pub weapon: NetworkResourceKey<24>,
    pub freeze: NetworkResourceKey<24>,
    pub ninja: NetworkResourceKey<24>,
    pub game: NetworkResourceKey<24>,
    pub ctf: NetworkResourceKey<24>,
    pub hud: NetworkResourceKey<24>,
    pub entities: NetworkResourceKey<24>,
    pub emoticons: NetworkResourceKey<24>,
    pub particles: NetworkResourceKey<24>,
    pub hook: NetworkResourceKey<24>,
}

impl NetworkCharacterInfo {
    // only provide a default that makes clear you used default
    pub fn explicit_default() -> Self {
        Self {
            name: NetworkString::new("TODO").unwrap(),
            clan: NetworkString::new("TODO").unwrap(),
            flag: NetworkString::new("default").unwrap(),

            skin_info: NetworkSkinInfo::Original,

            skin: "default".try_into().unwrap(),
            weapon: "default".try_into().unwrap(),
            ninja: "default".try_into().unwrap(),
            freeze: "default".try_into().unwrap(),
            game: "default".try_into().unwrap(),
            ctf: "default".try_into().unwrap(),
            hud: "default".try_into().unwrap(),
            entities: "default".try_into().unwrap(),
            emoticons: "default".try_into().unwrap(),
            particles: "default".try_into().unwrap(),
            hook: "default".try_into().unwrap(),
        }
    }
}
