use base::hash::Hash;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DefaultOnError;

pub struct ServerBrowserFilter {
    pub search: String,
    pub exclude: String,
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
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub country: i32,
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
    pub passworded: bool,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub cert_sha256_fingerprint: Hash,
}

#[derive(Debug)]
pub struct ServerBrowserServer {
    pub info: ServerBrowserInfo,
    pub address: String,
}

pub struct ServerBrowserData {
    pub servers: Vec<ServerBrowserServer>,
    pub filter: ServerBrowserFilter,
    pub cur_address: String,
    pub cur_cert_hash: Option<Hash>,
}

impl ServerBrowserData {
    pub fn servers_filtered(&self) -> impl Iterator<Item = &ServerBrowserServer> {
        self.servers.iter().filter(|server| {
            server
                .info
                .map
                .name
                .to_lowercase()
                .contains(&self.filter.search.to_lowercase())
                || server
                    .info
                    .name
                    .to_lowercase()
                    .contains(&self.filter.search.to_lowercase())
        })
    }
}
