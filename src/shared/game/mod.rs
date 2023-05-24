use crate::id_gen::{
    IDGenerator, IDGeneratorIDType, ID_GENERATOR_ID_FIRST, ID_GENERATOR_ID_INVALID,
};

pub mod collision;
pub mod entities;
pub mod simulation_pipe;
pub mod snapshot;
pub mod stage;
pub mod state;
pub mod weapons;
pub mod world;

/// The game element id is a unique identifier to help
/// creating a connecting from a network object and the actual game object
/// it should be unique per type
pub type TGameElementID = IDGeneratorIDType;
pub const INVALID_GAME_ELEMENT_ID: IDGeneratorIDType = ID_GENERATOR_ID_INVALID;
pub const FIRST_GAME_ELEMENT_ID: IDGeneratorIDType = ID_GENERATOR_ID_FIRST;

pub struct GameElementGenerator {
    stage_gen: IDGenerator,
    char_gen: IDGenerator,
}

impl Default for GameElementGenerator {
    fn default() -> Self {
        Self {
            stage_gen: IDGenerator::new(),
            char_gen: IDGenerator::new(),
        }
    }
}

impl GameElementGenerator {
    pub fn get_stage_id(&mut self) -> TGameElementID {
        self.stage_gen.get_next()
    }

    pub fn get_character_id(&mut self) -> TGameElementID {
        self.char_gen.get_next()
    }
}
