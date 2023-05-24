use super::{
    entities::{
        character::{Character, CharacterCore},
        Entity, EntityInterface,
    },
    simulation_pipe::{SimulationPipeEntities, SimulationPipeStage},
    GameElementGenerator, TGameElementID,
};

pub struct GameWorld {
    characters: Vec<Character>,
}

impl GameWorld {
    pub fn get_character_mut(&mut self, index: usize) -> &mut Character {
        &mut self.characters[index]
    }

    pub fn get_character_by_game_el_id(&mut self, id: &TGameElementID) -> Option<&Character> {
        self.characters
            .iter()
            .find(|c| c.base.game_element_id == *id)
    }

    pub fn get_characters(&self) -> &Vec<Character> {
        &self.characters
    }

    pub fn get_characters_mut(&mut self) -> &mut Vec<Character> {
        &mut self.characters
    }

    pub fn add_character(
        &mut self,
        game_el_gen: &mut GameElementGenerator,
        player_id: &TGameElementID,
    ) -> &mut Character {
        let _index = self.characters.len();
        self.characters
            .push(Character::new(&game_el_gen.get_character_id(), player_id));
        self.characters.last_mut().unwrap()
    }

    pub fn tick(&mut self, pipe: &mut SimulationPipeStage) {
        // todo move vector somewhere to decrease heap allocations
        let mut character_parts = Vec::<(&Entity, &mut CharacterCore)>::new();

        self.characters.iter_mut().for_each(|e| {
            if pipe.is_prediction {
                e.copy_core(pipe.next_core_index, pipe.prev_core_index);
            }
            character_parts.push(e.split_mut(pipe.next_core_index));
        });

        character_parts.iter_mut().for_each(|ent| {
            Character::tick(
                ent.0,
                ent.1,
                &mut SimulationPipeEntities {
                    player_inputs: pipe.player_input,
                    next_core_index: pipe.next_core_index,
                    prev_core_index: pipe.prev_core_index,
                    other_chars_after: &mut [],
                    other_chars_before: &mut [],
                    collision: pipe.collision,
                },
            );
        });
        character_parts.iter_mut().for_each(|ent| {
            Character::tick_deferred(
                ent.0,
                ent.1,
                &mut SimulationPipeEntities {
                    player_inputs: pipe.player_input,
                    next_core_index: pipe.next_core_index,
                    prev_core_index: pipe.prev_core_index,
                    other_chars_after: &mut [],
                    other_chars_before: &mut [],
                    collision: pipe.collision,
                },
            );
        });
    }
}

impl Default for GameWorld {
    fn default() -> GameWorld {
        GameWorld { characters: vec![] }
    }
}
