use std::{fmt::Display, num::NonZeroU16};

use hiarc::Hiarc;
use serde::{Deserialize, Deserializer, Serialize};

fn minus_one_deser<'de, D>(deserializer: D) -> Result<NonZeroU16, D::Error>
where
    D: Deserializer<'de>,
{
    let res = NonZeroU16::deserialize(deserializer)?;
    (res.get() != u16::MAX)
        .then_some(res)
        .ok_or_else(|| serde::de::Error::custom("value must be non zero and smaller than u16::MAX"))
}

/// this data type is specifically for tile layer's dimensions:
/// - 0 sized tile layers are not allowed, because they make no sense
/// - u16::MAX sized tile layers are not allowed, because a tile layer visual's last tile in a row/column
/// is exactly u16::MAX on it's right/bottom side
#[derive(
    Debug, Hiarc, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct NonZeroU16MinusOne(#[serde(deserialize_with = "minus_one_deser")] NonZeroU16);

impl NonZeroU16MinusOne {
    pub fn new(val: u16) -> Option<Self> {
        if val == u16::MAX {
            return None;
        }
        NonZeroU16::new(val).map(Self)
    }

    pub fn get(&self) -> u16 {
        self.0.get()
    }
}

impl Display for NonZeroU16MinusOne {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
