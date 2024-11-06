use std::{
    collections::HashMap,
    net::{SocketAddrV4, SocketAddrV6},
};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DefaultOnError;
use url::Url;

use super::communities::{Community, ServerIpList};

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Server {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub servers: HashMap<String, ServerIpList>,
}

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DdnetInfo {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub name: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub maps: Vec<String>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub points: i64,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub servers: Vec<Server>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(alias = "servers-kog")]
    pub servers_kog: Vec<Server>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub communities: Vec<Community>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(alias = "community-icons-download-url")]
    pub community_icons_download_url: Option<Url>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub news: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(alias = "map-download-url")]
    pub map_download_url: Option<Url>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub location: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub version: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(alias = "stun-servers-ipv6")]
    pub stun_servers_ipv6: Vec<SocketAddrV6>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(alias = "stun-servers-ipv4")]
    pub stun_servers_ipv4: Vec<SocketAddrV4>,
}
