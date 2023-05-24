use super::{simulation_pipe::SimulationPipeStage, world::GameWorld, TGameElementID};

/**
* The game stage represents a well split state of a complete world, which is useful to
* have multiple people being able to play on the same server without touching each other
* It's there to implement ddrace teams.
*/
pub struct GameStage {
    world: GameWorld,
    stage_index: u32,

    pub game_element_id: TGameElementID,
}

impl GameStage {
    pub fn new(stage_index: u32, game_element_id: TGameElementID) -> Self {
        Self {
            world: GameWorld::default(),
            stage_index: stage_index,

            game_element_id,
        }
    }

    pub fn get_world_mut(&mut self) -> &mut GameWorld {
        &mut self.world
    }

    pub fn get_world(&self) -> &GameWorld {
        &self.world
    }

    pub fn tick(&mut self, pipe: &mut SimulationPipeStage) {
        self.world.tick(pipe);
    }
}
