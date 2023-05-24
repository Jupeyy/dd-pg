use super::{simulation_pipe::SimulationPipeEntities, TGameElementID};
pub trait EntityInterface<C: Copy + Clone + bincode::Encode + bincode::Decode> {
    fn pre_tick(ent: &Entity, core: &mut C, pipe: &mut SimulationPipeEntities);
    fn tick(ent: &Entity, core: &mut C, pipe: &mut SimulationPipeEntities);
    fn tick_deferred(ent: &Entity, core: &mut C, pipe: &mut SimulationPipeEntities);

    // split the entity to all main objects it contains of
    // core (must be Copy'able)
    fn split_mut(self: &mut Self, index: usize) -> (&Entity, &mut C);

    // copy the core
    fn get_core_mut(self: &mut Self, index: usize) -> &mut C;
    fn copy_core(self: &mut Self, dst_index: usize, src_index: usize) {
        *self.get_core_mut(dst_index) = *self.get_core_mut(src_index);
    }
}

pub struct Entity {
    pub game_element_id: TGameElementID,
}

impl Entity {
    pub fn new(game_el_id: &TGameElementID) -> Self {
        Self {
            game_element_id: *game_el_id,
        }
    }
}

pub mod character;
pub mod character_core;
