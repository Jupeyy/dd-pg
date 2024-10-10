use std::collections::BTreeMap;

use game_interface::votes::MapVote;
use hiarc::{hiarc_safer_rc_refcell, Hiarc};

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc, Default)]
pub struct Votes {
    map_votes: BTreeMap<String, MapVote>,
    need_map_votes: bool,
}

#[hiarc_safer_rc_refcell]
impl Votes {
    pub fn request_map_votes(&mut self) {
        self.need_map_votes = true;
    }

    /// Automatically resets the "need" state, so
    /// another [`Votes::request_map_votes`] has to
    /// be called.
    pub fn needs_map_votes(&mut self) -> bool {
        std::mem::replace(&mut self.need_map_votes, false)
    }

    pub fn fill_map_votes(&mut self, map_votes: BTreeMap<String, MapVote>) {
        self.map_votes = map_votes;
    }

    pub fn collect_map_votes(&self) -> BTreeMap<String, MapVote> {
        self.map_votes.clone()
    }
}
