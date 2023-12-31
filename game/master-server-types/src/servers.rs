use serde::{Deserialize, Serialize};
use serde_json as json;

use crate::{addr::Addr, locations::Location};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
    pub addresses: Vec<Addr>,
    pub info_serial: i64,
    pub info: Box<json::value::RawValue>,
}

#[derive(Debug, Serialize)]
pub struct SerializedServers<'a> {
    pub servers: Vec<SerializedServer<'a>>,
}

impl<'a> SerializedServers<'a> {
    pub fn new() -> SerializedServers<'a> {
        SerializedServers {
            servers: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializeServerImpl<A, B> {
    pub addresses: A,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    pub info: B,
}

pub type SerializedServer<'a> = SerializeServerImpl<&'a [Addr], &'a json::value::RawValue>;

impl<'a> SerializedServer<'a> {
    pub fn new(server: &'a Server, location: Option<Location>) -> SerializedServer<'a> {
        SerializedServer {
            addresses: &server.addresses,
            location,
            info: &server.info,
        }
    }
}

pub type BrowserServer = SerializeServerImpl<Vec<Addr>, Box<json::value::RawValue>>;

impl BrowserServer {
    pub fn new(server: Server, location: Option<Location>) -> Self {
        Self {
            addresses: server.addresses,
            location,
            info: server.info,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserServers {
    pub servers: Vec<BrowserServer>,
}
