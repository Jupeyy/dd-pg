use std::collections::HashMap;

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// An entry on a http server
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct AssetIndexEntry {
    pub ty: String,
    pub hash: base::hash::Hash,
}

pub type AssetsIndex = HashMap<String, AssetIndexEntry>;
