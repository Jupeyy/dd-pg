pub mod stage {
    use std::{num::NonZeroU16, rc::Rc};

    use game_interface::types::{game::GameEntityId, id_gen::IdGenerator};
    use hashlink::LinkedHashMap;
    use hiarc::Hiarc;
    use math::math::vector::ubvec4;

    use crate::{
        game_objects::game_objects::GameObjectDefinitions,
        match_manager::match_manager::MatchManager,
        match_state::match_state::MatchState,
        simulation_pipe::simulation_pipe::{SimulationStageEvents, SimulationWorldEvents},
        types::types::GameOptions,
    };

    use super::super::{
        simulation_pipe::simulation_pipe::SimulationPipeStage,
        world::world::{GameWorld, WorldPool},
    };

    /// The game stage represents a well split state of a complete world, which is useful to
    /// have multiple people being able to play on the same server without touching each other.
    ///
    /// It's there to implement ddrace teams.
    #[derive(Debug, Hiarc)]
    pub struct GameStage {
        pub world: GameWorld,
        pub match_manager: MatchManager,
        pub stage_name: String,
        pub stage_color: ubvec4,

        pub(crate) simulation_events: SimulationStageEvents,

        game_object_definitions: Rc<GameObjectDefinitions>,
        pub game_element_id: GameEntityId,
    }

    impl GameStage {
        pub fn new(
            stage_name: String,
            stage_color: ubvec4,
            game_element_id: GameEntityId,
            world_pool: &WorldPool,
            game_object_definitions: &Rc<GameObjectDefinitions>,
            width: NonZeroU16,
            height: NonZeroU16,
            id_gen: Option<&IdGenerator>,
            game_options: GameOptions,
        ) -> Self {
            let simulation_events = SimulationStageEvents::new();
            Self {
                world: GameWorld::new(world_pool, game_object_definitions, width, height, id_gen),
                match_manager: MatchManager::new(game_options, &simulation_events),
                stage_name,
                stage_color,
                simulation_events,

                game_object_definitions: game_object_definitions.clone(),

                game_element_id,
            }
        }

        #[must_use]
        pub fn tick(&mut self, pipe: &mut SimulationPipeStage) -> SimulationWorldEvents {
            self.match_manager.game_match.tick();

            if let MatchState::Running { .. } = self.match_manager.game_match.state {
                self.simulation_events
                    .push_entity_evs(self.world.tick(pipe));
            }
            if !pipe.is_prediction && self.match_manager.tick(&mut self.world) {
                self.world = GameWorld::new(
                    &self.world.world_pool,
                    &self.game_object_definitions,
                    self.world.play_field.width(),
                    self.world.play_field.height(),
                    self.world.id_generator.as_ref(),
                );
                let game_options = self.match_manager.game_options;
                self.match_manager = MatchManager::new(game_options, &self.simulation_events);
            }

            self.simulation_events.take()
        }
    }

    pub type Stages = LinkedHashMap<GameEntityId, GameStage>;
}
