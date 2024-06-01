use hiarc::Hiarc;
use math::math::vector::ubvec4;
use serde::{Deserialize, Serialize};

use crate::types::resource_key::NetworkResourceKey;

use super::network_string::NetworkString;

// # network part
#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct NetworkSkinInfo {
    pub color: ubvec4,
}

#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct NetworkCharacterInfo {
    pub name: NetworkString<16>,
    pub clan: NetworkString<12>,
    pub country: NetworkString<3>,
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
            country: NetworkString::new("ENG").unwrap(),

            skin_info: NetworkSkinInfo {
                color: ubvec4::new(255, 255, 255, 255),
            },

            skin: "TODO".try_into().unwrap(),
            weapon: "TODO".try_into().unwrap(),
            ninja: "TODO".try_into().unwrap(),
            freeze: "TODO".try_into().unwrap(),
            game: "TODO".try_into().unwrap(),
            ctf: "TODO".try_into().unwrap(),
            hud: "TODO".try_into().unwrap(),
            entities: "TODO".try_into().unwrap(),
            emoticons: "TODO".try_into().unwrap(),
            particles: "TODO".try_into().unwrap(),
            hook: "TODO".try_into().unwrap(),
        }
    }
}
