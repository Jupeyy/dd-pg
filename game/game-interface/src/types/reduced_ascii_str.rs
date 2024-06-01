use std::{
    ops::{Deref, DerefMut},
    path::Path,
    str::FromStr,
};

use base::hash::Hash;
use hiarc::Hiarc;
use serde::{de, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReducedAsciiStringError {
    #[error("The string contained an unsupported character \"{0}\"")]
    InvalidCharacter(char),
    #[error("The string is part of reserved words: {0}")]
    ReservedWord(String),
    #[error("The string is similar to a file with hash: \"<name>_<hash>\". This is not allowed")]
    HashLike,
    #[error("Failed to convert from a standard string: {0}")]
    ConversionFromStringFailed(ascii::AsAsciiStrError),
}

/// A string that is purely ascii and additionally is limited to the following
/// char set:
/// - `[0-9]`
/// - `[a-z]`
/// - `[A-Z]`
/// - `_`, ` ` (space)
/// - Not allowed are reserved file names
/// - Not allowed are hash like names: <name>_<hash>  
/// One guarantee this should give is, that a string can safely passed as name
/// for a file path without changing directory or simiar.
#[derive(Debug, Default, Hiarc, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ReducedAsciiString(ascii::AsciiString);

impl Deref for ReducedAsciiString {
    type Target = ascii::AsciiString;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for ReducedAsciiString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ReducedAsciiString {
    pub fn new(s: impl Into<ascii::AsciiString>) -> Result<Self, ReducedAsciiStringError> {
        let s = s.into();
        Self::is_valid(&s)?;
        Ok(Self(s))
    }

    pub fn from_str_lossy(s: &str) -> Self {
        let mut s = ascii::AsciiString::from_str(
            s.chars()
                .filter(|char| {
                    let Ok(char) = ascii::AsciiChar::from_ascii(*char) else {
                        return false;
                    };
                    Self::is_char_valid(&char)
                })
                .collect::<String>()
                .as_str(),
        )
        .unwrap();
        if Self::is_reserved_word(s.as_str()).is_err() {
            s = Default::default();
        }
        if Self::is_hash_like(s.as_str()).is_err() {
            s = Default::default();
        }
        Self(s)
    }

    fn is_char_valid(char: &ascii::AsciiChar) -> bool {
        char.is_alphabetic()
            || char.is_ascii_digit()
            || *char == ascii::AsciiChar::new('_')
            || *char == ascii::AsciiChar::new(' ')
    }

    fn is_reserved_word(s: &str) -> Result<(), String> {
        match s.to_lowercase().as_str() {
            "con" | "prn" | "aux" | "nul" | "lst" | "com1" | "com2" | "com3" | "com4" | "com5"
            | "com6" | "com7" | "com8" | "com9" | "com0" | "lpt1" | "lpt2" | "lpt3" | "lpt4"
            | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9" | "lpt0" => {
                Err(format!("\"{s}\" is a reserved file name on Windows"))
            }
            _ => Ok(()),
        }
    }

    fn is_hash_like(s: &str) -> Result<(), ()> {
        let path: &Path = s.as_ref();
        if let Some((_, name_hash)) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit_once('_'))
        {
            if name_hash.len() == Hash::default().len()
                && name_hash.find(|c: char| !c.is_ascii_hexdigit()).is_none()
            {
                return Err(());
            }
        }
        Ok(())
    }

    pub fn is_valid(s: &ascii::AsciiString) -> Result<(), ReducedAsciiStringError> {
        if let Some(char) = s.chars().find(|char| !Self::is_char_valid(char)) {
            Err(ReducedAsciiStringError::InvalidCharacter(char.as_char()))
        } else {
            Self::is_reserved_word(s.as_str()).map_err(ReducedAsciiStringError::ReservedWord)?;
            Self::is_hash_like(s.as_str()).map_err(|_| ReducedAsciiStringError::HashLike)?;
            Ok(())
        }
    }
}

impl<'de> de::Deserialize<'de> for ReducedAsciiString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        <ascii::AsciiString as de::Deserialize>::deserialize(deserializer).and_then(|inner| {
            Self::is_valid(&inner)
                .map_err(|err| match err {
                    ReducedAsciiStringError::InvalidCharacter(char) => de::Error::invalid_value(
                        de::Unexpected::Char(char),
                        &"expected a pure ascii string with reduced character set ([A-Z,a-z,0-9] & \"_ \")",
                    ),
                    err => de::Error::custom(format!("{err}"))
                })
                .map(|_| Self(inner))
        })
    }
}

impl TryFrom<&str> for ReducedAsciiString {
    type Error = ReducedAsciiStringError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(
            ascii::AsciiString::from_str(value)
                .map_err(ReducedAsciiStringError::ConversionFromStringFailed)?,
        )
    }
}
