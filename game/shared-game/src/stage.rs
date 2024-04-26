pub mod stage {
    use std::{num::NonZeroU16, rc::Rc, sync::Arc};

    use base_log::log::SystemLog;
    use game_interface::types::{game::GameEntityId, id_gen::IdGenerator};
    use hashlink::LinkedHashMap;
    use hiarc::Hiarc;

    use crate::{
        game_objects::game_objects::GameObjectDefinitions,
        match_manager::match_manager::{MatchManager, MatchState},
        simulation_pipe::simulation_pipe::{SimulationStageEvents, SimulationWorldEvents},
        types::types::GameOptions,
    };

    use super::super::{
        simulation_pipe::simulation_pipe::SimulationPipeStage,
        world::world::{GameWorld, WorldPool},
    };

    /// The game stage represents a well split state of a complete world, which is useful to
    /// have multiple people being able to play on the same server without touching each other
    /// It's there to implement ddrace teams.
    #[derive(Debug, Hiarc)]
    pub struct GameStage {
        pub world: GameWorld,
        pub match_manager: MatchManager,
        pub stage_index: usize,

        pub(crate) simulation_events: SimulationStageEvents,

        game_object_definitions: Rc<GameObjectDefinitions>,
        pub game_element_id: GameEntityId,
    }

    impl GameStage {
        pub fn new(
            stage_index: usize,
            game_element_id: GameEntityId,
            world_pool: &WorldPool,
            game_object_definitions: &Rc<GameObjectDefinitions>,
            width: NonZeroU16,
            height: NonZeroU16,
            id_gen: &IdGenerator,
            game_options: GameOptions,
            log: &Arc<SystemLog>,
        ) -> Self {
            let simulation_events = SimulationStageEvents::new();
            Self {
                world: GameWorld::new(
                    world_pool,
                    game_object_definitions,
                    width,
                    height,
                    id_gen,
                    log,
                ),
                match_manager: MatchManager::new(game_element_id, game_options, &simulation_events),
                stage_index: stage_index,
                simulation_events,

                game_object_definitions: game_object_definitions.clone(),

                game_element_id,
            }
        }

        #[must_use]
        pub fn tick(&mut self, pipe: &mut SimulationPipeStage) -> SimulationWorldEvents {
            if let MatchState::Running = self.match_manager.game_match.state {
                self.simulation_events
                    .push_entity_evs(self.world.tick(pipe));
            }
            if self.match_manager.handle_events(&mut self.world) {
                self.world = GameWorld::new(
                    &self.world.world_pool,
                    &self.game_object_definitions,
                    self.world.play_field.width(),
                    self.world.play_field.height(),
                    &self.world.id_gen,
                    &self.world.log,
                )
            }

            self.simulation_events.take()
        }
    }

    pub type Stages = LinkedHashMap<GameEntityId, GameStage>;
}
