use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CompressHeader {
    pub size: usize,
    pub is_compressed: bool,
}
