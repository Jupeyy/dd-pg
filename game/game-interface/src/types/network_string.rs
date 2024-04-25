use hiarc::Hiarc;
use serde::{de, Serialize};
use std::ops::Deref;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkStringError {
    #[error("The unicode char length exceeded the allowed maximum length")]
    InvalidLength,
}

/// A string that that checks the max __unicode__ (code points) length
/// of a string at deserialization & creation time
#[derive(Debug, Default, Hiarc, Clone, Hash, Serialize)]
pub struct NetworkString<const MAX_LENGTH: usize>(String);

impl<const MAX_LENGTH: usize> Deref for NetworkString<MAX_LENGTH> {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const MAX_LENGTH: usize> NetworkString<MAX_LENGTH> {
    pub fn new(s: impl Into<String>) -> Result<Self, NetworkStringError> {
        let s = s.into();
        if s.chars().count() > MAX_LENGTH {
            Err(NetworkStringError::InvalidLength)
        } else {
            Ok(Self(s))
        }
    }
}

impl<'de, const MAX_LENGTH: usize> de::Deserialize<'de> for NetworkString<MAX_LENGTH> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        <String as de::Deserialize>::deserialize(deserializer).and_then(|inner| {
            if inner.chars().count() > MAX_LENGTH {
                Err(de::Error::invalid_length(
                    inner.chars().count(),
                    &"a unicode char length lower than the maximum",
                ))
            } else {
                Ok(Self(inner))
            }
        })
    }
}
