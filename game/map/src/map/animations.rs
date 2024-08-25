use std::time::Duration;

use hiarc::Hiarc;
use is_sorted::IsSorted;
use math::math::vector::{fvec3, nffixed, nfvec4, vec1_base};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnimPointCurveType {
    Step = 0,
    Linear,
    Slow,
    Fast,
    Smooth,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct AnimPoint<T> {
    pub time: Duration,
    pub curve_type: AnimPointCurveType,
    pub value: T,
}

impl<T> PartialEq for AnimPoint<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<T> PartialOrd for AnimPoint<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time.partial_cmp(&other.time)
    }
}

pub type AnimPointPos = AnimPoint<fvec3>;
pub type AnimPointColor = AnimPoint<nfvec4>;
pub type AnimPointSound = AnimPoint<vec1_base<nffixed>>;

fn points_deser<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + PartialOrd,
{
    let res = <Vec<T>>::deserialize(deserializer)?;
    IsSorted::is_sorted(&mut res.iter())
        .then_some(res)
        .ok_or_else(|| serde::de::Error::custom("vec not sorted"))
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct AnimBase<T: DeserializeOwned + PartialOrd> {
    #[serde(deserialize_with = "points_deser")]
    pub points: Vec<T>,

    pub synchronized: bool,

    /// optional name, mostly intersting for editor
    pub name: String,
}

pub type PosAnimation = AnimBase<AnimPointPos>;
pub type ColorAnimation = AnimBase<AnimPointColor>;
pub type SoundAnimation = AnimBase<AnimPointSound>;

#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct Animations {
    pub pos: Vec<PosAnimation>,
    pub color: Vec<ColorAnimation>,
    pub sound: Vec<SoundAnimation>,
}
