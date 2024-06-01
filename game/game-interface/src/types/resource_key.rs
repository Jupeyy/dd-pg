use std::str::FromStr;

use hiarc::Hiarc;
use pool::{pool::Pool, recycle::Recycle, traits::Recyclable};
use serde::{Deserialize, Serialize};

use crate::types::network_string::{NetworkAsciiStringError, NetworkReducedAsciiString};

use super::reduced_ascii_str::{ReducedAsciiString, ReducedAsciiStringError};

/// The resource key is the general way to describe
/// names of resources like skins or weapons etc.
/// It must contain a string name
/// (reduced to a limited character set [ReducedAsciiString])
/// and an optional hash.
/// If the hash is not used then the client
/// automatically decides which resource to load
/// If the hash exists, it __only__ loads/shows the
/// if a resource with that hash exists (or can
/// be downloaded).
#[derive(Debug, Hiarc, Default, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ResourceKey {
    pub name: ReducedAsciiString,
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
    pub fn from_str_lossy(s: &str) -> Self {
        ResourceKey {
            name: ReducedAsciiString::from_str_lossy(s),
            hash: None,
        }
    }

    pub fn eq_str(&self, s: &ascii::AsciiStr) -> bool {
        self.name.as_str() == s.as_str() && self.hash.is_none()
    }

    pub fn clone_from_network<const MAX_LENGTH: usize>(
        &mut self,
        n: &NetworkResourceKey<MAX_LENGTH>,
    ) {
        self.hash = n.hash;
        self.name.clone_from(&n.name);
    }
}

impl From<ReducedAsciiString> for ResourceKey {
    fn from(value: ReducedAsciiString) -> Self {
        Self {
            hash: None,
            name: value,
        }
    }
}

impl TryFrom<&str> for ResourceKey {
    type Error = ReducedAsciiStringError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self {
            name: ReducedAsciiString::new(
                ascii::AsciiString::from_str(value)
                    .map_err(ReducedAsciiStringError::ConversionFromStringFailed)?,
            )?,

            ..Default::default()
        })
    }
}

/// A resource key that that checks the max length & only gives a reduced
/// ascii character set of a string at deserialization & creation time.
/// See [NetworkReducedAsciiString]
#[derive(Debug, Default, Hiarc, Clone, Hash, Serialize, Deserialize)]
pub struct NetworkResourceKey<const MAX_LENGTH: usize> {
    pub name: NetworkReducedAsciiString<MAX_LENGTH>,
    pub hash: Option<base::hash::Hash>,
}

impl<const MAX_LENGTH: usize> NetworkResourceKey<MAX_LENGTH> {
    pub fn new(s: impl Into<ReducedAsciiString>) -> Result<Self, NetworkAsciiStringError> {
        Ok(Self {
            hash: None,
            name: NetworkReducedAsciiString::new(s)?,
        })
    }

    pub fn from_str_lossy(s: &str) -> Self {
        NetworkResourceKey {
            name: NetworkReducedAsciiString::from_str_lossy(s),
            hash: None,
        }
    }
}

impl<const MAX_LENGTH: usize> TryFrom<&ascii::AsciiStr> for NetworkResourceKey<MAX_LENGTH> {
    type Error = NetworkAsciiStringError;
    fn try_from(value: &ascii::AsciiStr) -> Result<Self, Self::Error> {
        Self::new(
            ReducedAsciiString::new(value).map_err(NetworkAsciiStringError::RedcuedAsciiStrErr)?,
        )
    }
}

impl<const MAX_LENGTH: usize> TryFrom<&str> for NetworkResourceKey<MAX_LENGTH> {
    type Error = NetworkAsciiStringError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(
            ReducedAsciiString::new(ascii::AsciiString::from_str(value).map_err(|err| {
                NetworkAsciiStringError::RedcuedAsciiStrErr(
                    ReducedAsciiStringError::ConversionFromStringFailed(err),
                )
            })?)
            .map_err(NetworkAsciiStringError::RedcuedAsciiStrErr)?,
        )
    }
}

impl<const MAX_LENGTH: usize> TryFrom<ReducedAsciiString> for NetworkResourceKey<MAX_LENGTH> {
    type Error = NetworkAsciiStringError;

    fn try_from(value: ReducedAsciiString) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

pub type PoolResourceKey = Recycle<ResourceKey>;
pub type ResourceKeyPool = Pool<ResourceKey>;
