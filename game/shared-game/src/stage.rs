pub mod stage {
    use shared_base::game_types::TGameElementID;

    use super::super::{
        simulation_pipe::simulation_pipe::SimulationPipeStage,
        world::world::{GameWorld, WorldPool},
    };

    /**
     * The game stage represents a well split state of a complete world, which is useful to
     * have multiple people being able to play on the same server without touching each other
     * It's there to implement ddrace teams.
     */
    pub struct GameStage {
        pub world: GameWorld,
        pub stage_index: usize,

        pub game_element_id: TGameElementID,
    }

    impl GameStage {
        pub fn new(
            stage_index: usize,
            game_element_id: TGameElementID,
            character_pool: &mut WorldPool,
        ) -> Self {
            Self {
                world: GameWorld::new(character_pool),
                stage_index: stage_index,

                game_element_id,
            }
        }

        pub fn tick(&mut self, pipe: &mut SimulationPipeStage) {
            self.world.tick(pipe);
        }
    }
}
