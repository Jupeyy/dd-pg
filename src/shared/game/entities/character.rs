use crate::game::{
    simulation_pipe::{LocalPlayerInput, SimulationPipeEntities},
    TGameElementID,
};

use super::{
    character_core::{Core, CorePhysics, CorePipe},
    Entity, EntityInterface,
};

use bincode::{Decode, Encode};
use math::math::vector::vec2;

#[derive(Copy, Clone, Default, Encode, Decode)]
pub struct CharacterCore {
    pub core: Core,
    pub player_id: TGameElementID,
}

pub struct Character {
    pub base: Entity,
    pub cores: [CharacterCore; 2],
}

impl Character {
    pub fn new(game_el_id: &TGameElementID, player_id: &TGameElementID) -> Self {
        Self {
            base: Entity::new(game_el_id),
            cores: [
                CharacterCore {
                    core: Core::default(),
                    player_id: *player_id,
                },
                CharacterCore {
                    core: Core::default(),
                    player_id: *player_id,
                },
            ],
        }
    }
}

pub struct CorePipeStr<'a, 'b> {
    pub input: &'a LocalPlayerInput,
    pub other_chars_before: &'b mut &'a mut [&'a mut Core],
    pub other_chars_after: &'b mut &'a mut [&'a mut Core],
}

impl<'a, 'b> CorePipe for CorePipeStr<'a, 'b> {
    fn input_target_x(&self) -> i32 {
        self.input.x
    }

    fn input_target_y(&self) -> i32 {
        self.input.y
    }

    fn input_dir(&self) -> i32 {
        self.input.dir
    }

    fn input_jump(&self) -> bool {
        self.input.jump
    }

    fn input_hook(&self) -> bool {
        self.input.hook
    }

    fn tick_speed(&self) -> crate::types::GameTickType {
        50 // TODO
    }

    fn get_character_core(&mut self, index: usize) -> Option<&mut Core> {
        if index < self.other_chars_before.len() {
            return Some(self.other_chars_before[index]);
        }
        if index > self.other_chars_before.len() + 1
            && index < self.other_chars_after.len() + (self.other_chars_before.len() + 1)
        {
            return Some(self.other_chars_after[index - (self.other_chars_before.len() + 1)]);
        }
        None
    }

    fn intersect_line_tele_hook(
        &self,
        _pos0: &vec2,
        _pos1: &vec2,
        _out_collision: &mut vec2,
        _out_before_collision: &mut vec2,
        _tele_nr: &mut i32,
    ) -> u8 {
        0 // TODO
    }
}

impl CorePhysics for Character {}

impl EntityInterface<CharacterCore> for Character {
    fn pre_tick(_ent: &Entity, _core: &mut CharacterCore, _pipe: &mut SimulationPipeEntities) {}

    fn tick(_ent: &Entity, core: &mut CharacterCore, pipe: &mut SimulationPipeEntities) {
        let mut core_pipe = CorePipeStr {
            input: pipe.player_inputs.get_input(core.player_id).unwrap(),
            other_chars_before: &mut pipe.other_chars_before,
            other_chars_after: &mut pipe.other_chars_after,
        };
        Self::physics_tick(&mut core.core, true, true, &mut core_pipe, pipe.collision);
    }

    fn tick_deferred(_ent: &Entity, core: &mut CharacterCore, pipe: &mut SimulationPipeEntities) {
        let mut core_pipe = CorePipeStr {
            input: pipe.player_inputs.get_input(core.player_id).unwrap(),
            other_chars_before: &mut pipe.other_chars_before,
            other_chars_after: &mut pipe.other_chars_after,
        };
        Self::physics_move(&mut core.core, &mut core_pipe, &pipe.collision);
        Self::physics_quantize(&mut core.core);
    }

    fn split_mut(self: &mut Self, index: usize) -> (&Entity, &mut CharacterCore) {
        (&self.base, &mut self.cores[index])
    }

    fn get_core_mut(self: &mut Self, index: usize) -> &mut CharacterCore {
        &mut self.cores[index]
    }
}
