use game_interface::types::input::{CharacterInput, CharacterInputConsumableDiff};
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// Usually this class does not need to be used inside the physics
#[derive(Debug, Hiarc, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub inp: CharacterInput,
    version: u64,
}

impl PlayerInput {
    /// tries to overwrite the current input, returns a consumable
    /// input diff if it was overwritten
    pub fn try_overwrite(
        &mut self,
        inp: &CharacterInput,
        version: u64,
        force: bool,
    ) -> Option<CharacterInputConsumableDiff> {
        if self.version < version || force {
            let res = inp.consumable.diff(&self.inp.consumable);

            self.inp = *inp;
            self.version = version;

            Some(res)
        } else {
            None
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn inc_version(&mut self) {
        self.version += 1;
    }
}
