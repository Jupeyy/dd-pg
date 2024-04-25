use num_derive::FromPrimitive;
use strum_macros::IntoStaticStr;

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, FromPrimitive, IntoStaticStr,
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
pub const EMOTICONS_COUNT: usize = EmoticonType::QUESTION as usize + 1;
