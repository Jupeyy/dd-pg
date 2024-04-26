use hiarc::Hiarc;
use pool::{pool::Pool, recycle::Recycle, traits::Recyclable};
use serde::{Deserialize, Serialize};

use crate::types::network_string::{NetworkString, NetworkStringError};

/// The resource key is the general way to describe
/// names of resources like skins or weapons etc.
/// It must contain a string name and an optional
/// hash. If the hash is not used then the client
/// automatically decides which resource to load
/// If the hash exists, it __only__ loads/shows the
/// if a resource with that hash exists (or can
/// be downloaded).
#[derive(Debug, Hiarc, Default, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ResourceKey {
    pub name: String,
    pub hash: Option<base::hash::Hash>,
}

impl Recyclable for ResourceKey {
    fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self) {
        self.name.clear();
        self.hash = None;
    }
}

impl ResourceKey {
    pub fn eq_str(&self, s: &str) -> bool {
        self.name == s && self.hash.is_none()
    }

    pub fn clone_from_network<const MAX_LENGTH: usize>(
        &mut self,
        n: &NetworkResourceKey<MAX_LENGTH>,
    ) {
        self.hash = n.hash;
        self.name.clone_from(&n.name);
    }
}

impl From<&str> for ResourceKey {
    fn from(value: &str) -> Self {
        Self {
            hash: None,
            name: value.into(),
        }
    }
}

impl From<String> for ResourceKey {
    fn from(value: String) -> Self {
        Self {
            hash: None,
            name: value,
        }
    }
}

/// A resource key that that checks the max __unicode__ (code points) length
/// of a string at deserialization & creation time
#[derive(Debug, Default, Hiarc, Clone, Hash, Serialize, Deserialize)]
pub struct NetworkResourceKey<const MAX_LENGTH: usize> {
    pub name: NetworkString<MAX_LENGTH>,
    pub hash: Option<base::hash::Hash>,
}

impl<const MAX_LENGTH: usize> NetworkResourceKey<MAX_LENGTH> {
    pub fn new(s: impl Into<String>) -> Result<Self, NetworkStringError> {
        Ok(Self {
            hash: None,
            name: NetworkString::new(s)?,
        })
    }
}

impl<const MAX_LENGTH: usize> TryFrom<&str> for NetworkResourceKey<MAX_LENGTH> {
    type Error = NetworkStringError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<const MAX_LENGTH: usize> TryFrom<String> for NetworkResourceKey<MAX_LENGTH> {
    type Error = NetworkStringError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

pub type PoolResourceKey = Recycle<ResourceKey>;
pub type ResourceKeyPool = Pool<ResourceKey>;
