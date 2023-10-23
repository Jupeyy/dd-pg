use serde::{Deserialize, Serialize};

/// this represents the id of any kind of identifiable resource
/// be it characters, players, stages, projectiles etc.
/// Note: It is purposely not copyable for debug reasons, even if it release it is
/// a simple u64. Just let the compiler optimize this
#[derive(
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Copy, // TODO remove, character core annoying rn
    Clone,
    Hash,
    bincode::Encode,
    bincode::Decode,
)]
pub struct IDGeneratorIDType(pub u64); // TODO! change visibility to private again

pub const ID_GENERATOR_ID_INVALID: IDGeneratorIDType = IDGeneratorIDType(0);
pub const ID_GENERATOR_ID_FIRST: IDGeneratorIDType = IDGeneratorIDType(1);

impl Default for IDGeneratorIDType {
    fn default() -> Self {
        ID_GENERATOR_ID_INVALID
    }
}

#[derive(Clone)]
pub struct IDGenerator {
    cur_id: IDGeneratorIDType,
}

impl IDGenerator {
    pub fn new() -> Self {
        Self {
            cur_id: ID_GENERATOR_ID_FIRST,
        }
    }

    pub fn get_next(&mut self) -> IDGeneratorIDType {
        let cur = self.cur_id.clone();
        self.cur_id.0 += 1;
        cur
    }

    pub fn is_valid(&self, id: &IDGeneratorIDType) -> bool {
        id.0 != ID_GENERATOR_ID_INVALID.0
    }
}
