use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use serde_json as json;
use serde_with::serde_as;

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

impl<'a> Default for SerializedServers<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> SerializedServers<'a> {
    pub fn new() -> SerializedServers<'a> {
        SerializedServers {
            servers: Vec::new(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableServerImpl<A, B> {
    pub addresses: A,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    pub info: B,
}

pub type SerializedServer<'a> = SerializableServerImpl<&'a [Addr], &'a json::value::RawValue>;

impl<'a> SerializedServer<'a> {
    pub fn new(server: &'a Server, location: Option<Location>) -> SerializedServer<'a> {
        SerializedServer {
            addresses: &server.addresses,
            location,
            info: &server.info,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Addresses(#[serde_as(as = "serde_with::VecSkipError<_>")] pub Vec<Addr>);

impl Deref for Addresses {
    type Target = Vec<Addr>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Addresses {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub type BrowserServer = SerializableServerImpl<Addresses, Box<json::value::RawValue>>;

impl BrowserServer {
    pub fn new(server: Server, location: Option<Location>) -> Self {
        Self {
            addresses: Addresses(server.addresses),
            location,
            info: server.info,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserServers {
    pub servers: Vec<BrowserServer>,
}
