use base::hash::Hash;
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// a reference to a external resource
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapResourceRef {
    pub name: String,
    pub blake3_hash: Hash,
    pub ty: String,
}

/// a reference to a external resource
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct Resources {
    pub images: Vec<MapResourceRef>,
    /// images with certain restrictions (divisible by x/y)
    /// e.g. used for tile layers
    pub image_arrays: Vec<MapResourceRef>,
    pub sounds: Vec<MapResourceRef>,
}
