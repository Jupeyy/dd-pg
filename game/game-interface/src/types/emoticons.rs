use hiarc::Hiarc;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
pub use strum::{EnumCount, EnumIter, IntoEnumIterator, IntoStaticStr};

#[derive(
    Debug,
    Hiarc,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    FromPrimitive,
    EnumIter,
    EnumCount,
    IntoStaticStr,
    Serialize,
    Deserialize,
)]
pub enum EmoticonType {
    OOP,
    EXCLAMATION,
    HEARTS,
    DROP,
    DOTDOT,
    MUSIC,
    SORRY,
    GHOST,
    SUSHI,
    SPLATTEE,
    DEVILTEE,
    ZOMG,
    ZZZ,
    WTF,
    EYES,
    QUESTION,
}
