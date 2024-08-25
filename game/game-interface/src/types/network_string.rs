use hiarc::Hiarc;
use serde::{de, Serialize};
use std::ops::Deref;

use thiserror::Error;

use super::reduced_ascii_str::{ReducedAsciiString, ReducedAsciiStringError};

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

#[derive(Error, Debug)]
pub enum NetworkAsciiStringError {
    #[error("The ascii string length exceeded the allowed maximum length of {0}")]
    InvalidLength(usize),
    #[error("{0}")]
    RedcuedAsciiStrErr(ReducedAsciiStringError),
}

/// A string that is purely ascii and additionally is limited to the following
/// char set (see also [`ReducedAsciiString`]):
/// - `[0-9]`
/// - `[a-z]`
/// - `[A-Z]`
/// - `_`, ` ` (space)
/// - `MAX_LENGTH`
#[derive(Debug, Default, Hiarc, Clone, Hash, Serialize, PartialOrd, PartialEq, Eq)]
pub struct NetworkReducedAsciiString<const MAX_LENGTH: usize>(ReducedAsciiString);

impl<const MAX_LENGTH: usize> Deref for NetworkReducedAsciiString<MAX_LENGTH> {
    type Target = ReducedAsciiString;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const MAX_LENGTH: usize> NetworkReducedAsciiString<MAX_LENGTH> {
    pub fn is_valid(s: &ReducedAsciiString) -> Result<(), NetworkAsciiStringError> {
        if s.chars().count() > MAX_LENGTH {
            Err(NetworkAsciiStringError::InvalidLength(MAX_LENGTH))
        } else {
            ReducedAsciiString::is_valid(s).map_err(NetworkAsciiStringError::RedcuedAsciiStrErr)?;
            Ok(())
        }
    }

    pub fn new(
        s: impl TryInto<ReducedAsciiString, Error = ReducedAsciiStringError>,
    ) -> Result<Self, NetworkAsciiStringError> {
        let s = s
            .try_into()
            .map_err(NetworkAsciiStringError::RedcuedAsciiStrErr)?;
        Self::is_valid(&s)?;
        Ok(Self(s))
    }

    pub fn from_str_lossy(s: &str) -> Self {
        Self(ReducedAsciiString::from_str_lossy(s))
    }
}

impl<'de, const MAX_LENGTH: usize> de::Deserialize<'de> for NetworkReducedAsciiString<MAX_LENGTH> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        <ReducedAsciiString as de::Deserialize>::deserialize(deserializer).and_then(|inner| {
            Self::is_valid(&inner)
                .map_err(|err| match err {
                    NetworkAsciiStringError::InvalidLength(len) => de::Error::invalid_length(
                        inner.chars().count(),
                        &format!("a char length lower than the maximum: {len}").as_str(),
                    ),
                    NetworkAsciiStringError::RedcuedAsciiStrErr(ReducedAsciiStringError::InvalidCharacter(char)) => de::Error::invalid_value(
                        de::Unexpected::Char(char),
                        &"expected a pure ascii string with reduced character set ([A-Z,a-z,0-9] & \"_ \")",
                    ),
                    err => de::Error::custom(format!("{err}"))
                })
                .map(|_| Self(inner))
        })
    }
}

impl<const MAX_LENGTH: usize> TryFrom<&str> for NetworkReducedAsciiString<MAX_LENGTH> {
    type Error = NetworkAsciiStringError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<const MAX_LENGTH: usize> From<NetworkReducedAsciiString<MAX_LENGTH>> for ReducedAsciiString {
    fn from(value: NetworkReducedAsciiString<MAX_LENGTH>) -> Self {
        value.0
    }
}
