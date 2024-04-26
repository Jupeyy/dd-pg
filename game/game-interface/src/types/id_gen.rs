use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use serde::{Deserialize, Serialize};

/// this represents the id of any kind of identifiable resource
/// be it characters, stages, projectiles etc.
/// Note: It is purposely not copyable for debug reasons, even if it release it is
/// a simple u64. Just let the compiler optimize this
#[derive(Debug, Hiarc, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, Hash)]
pub struct IdGeneratorIdType(u64);

const ID_GENERATOR_ID_FIRST: IdGeneratorIdType = IdGeneratorIdType(1);

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct IdGenerator {
    cur_id: IdGeneratorIdType,
}

#[hiarc_safer_rc_refcell]
impl Default for IdGenerator {
    fn default() -> Self {
        Self {
            cur_id: ID_GENERATOR_ID_FIRST,
        }
    }
}

#[hiarc_safer_rc_refcell]
impl IdGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    /// generate the next unique id of this generator
    pub fn get_next(&mut self) -> IdGeneratorIdType {
        let cur = self.cur_id;
        self.cur_id.0 += 1;
        cur
    }
}
