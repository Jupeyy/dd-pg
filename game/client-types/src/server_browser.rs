use serde::Deserialize;
use serde_with::serde_as;
use serde_with::DefaultOnError;

pub struct ServerBrowserFilter {
    pub search: String,
    pub exclude: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ServerBrowserInfo {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub game_type: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub map: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub map_sha256: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub players: Vec<()>,
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
}
