use base::hash::Hash;
use game_interface::types::character_info::NetworkSkinInfo;
use game_interface::types::render::character::TeeEye;
use game_interface::types::resource_key::NetworkResourceKey;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DefaultOnError;

pub struct ServerBrowserFilter {
    pub search: String,
    pub exclude: String,
}

#[serde_as]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ServerBrowserSkin {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: NetworkResourceKey<24>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub info: NetworkSkinInfo,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub eye: TeeEye,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerBrowserPlayer {
    #[serde(alias = "time")]
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub score: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub skin: ServerBrowserSkin,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub clan: String,
    #[serde(alias = "country")]
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub flag: String,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ServerBrowserInfoMap {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub blake3: Hash,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub size: usize,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerBrowserInfo {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub game_type: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub version: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub map: ServerBrowserInfoMap,
    #[serde(alias = "clients")]
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub players: Vec<ServerBrowserPlayer>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub max_players: u32,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub passworded: bool,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub cert_sha256_fingerprint: Hash,
}

#[derive(Debug)]
pub struct ServerBrowserServer {
    pub info: ServerBrowserInfo,
    pub address: String,
    pub location: String,
}

pub struct ServerBrowserData {
    pub servers: Vec<ServerBrowserServer>,
}
