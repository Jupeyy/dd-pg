/// everything related to a single match/round/race-run
pub mod match_manager {
    use game_interface::events::GameWorldActionKillWeapon;
    use hiarc::{hi_closure, Hiarc};

    use crate::{
        entities::entity::entity::{DropMode, EntityInterface},
        events::events::{CharacterEvent, FlagEvent},
        match_state::match_state::{Match, MatchState, MatchType},
        simulation_pipe::simulation_pipe::{
            SimulationEventWorldEntityType, SimulationStageEvents, SimulationWorldEvent,
        },
        types::types::{GameOptions, GameType},
        world::world::GameWorld,
    };

    #[derive(Debug, Hiarc)]
    pub struct MatchManager {
        pub(crate) game_options: GameOptions,
        simulation_events: SimulationStageEvents,

        pub(crate) game_match: Match,
    }

    impl MatchManager {
        pub fn new(game_options: GameOptions, simulation_events: &SimulationStageEvents) -> Self {
            Self {
                game_match: Match {
                    ty: match game_options.ty {
                        GameType::Solo => MatchType::Solo,
                        GameType::Team => MatchType::Sided {
                            scores: Default::default(),
                        },
                    },
                    state: MatchState::Running {
                        round_ticks_passed: Default::default(),
                    },
                },
                game_options,
                simulation_events: simulation_events.clone(),
            }
        }

        fn handle_events(&mut self, world: &mut GameWorld) {
            let game_match = &mut self.game_match;
            let game_options = &self.game_options;
            self.simulation_events
                .for_each(hi_closure!([game_match: &mut Match, game_options: &GameOptions, world: &mut GameWorld], |ev: &SimulationWorldEvent| -> () {
                    match ev {
                        SimulationWorldEvent::Entity(entity_ev) => match &entity_ev.ev {
                            SimulationEventWorldEntityType::Character { ev, .. } => if let CharacterEvent::Despawn { killer_id, .. } = ev {
                                if let Some(char) = killer_id.and_then(|killer_id| world.characters.get_mut(&killer_id)) {
                                    char.core.score += 1;
                                    if let (MatchType::Sided { scores }, Some(team)) = (&mut game_match.ty, char.core.side) {
                                        scores[team as usize] += 1;
                                    }
                                    game_match.win_check(game_options, &[char]);
                                }
                            },
                            SimulationEventWorldEntityType::Flag { ev, .. } => {
                                match ev {
                                    FlagEvent::Capture { .. } => {
                                        if let Some(char) = entity_ev.owner_id.and_then(|character_id| world.characters.get_mut(&character_id)) {
                                            char.core.score += 5;
                                            if let (MatchType::Sided { scores }, Some(team)) = (&mut game_match.ty, char.core.side) {
                                                scores[team as usize] += 100;
                                            }
                                            game_match.win_check(game_options, &[char]);
                                        }
                                    },
                                    FlagEvent:: Despawn {
                                      ..
                                    } |
                                    FlagEvent:: Sound {
                                        ..
                                    } |
                                    FlagEvent::Effect {
                                        ..
                                    } => {
                                        // ignore
                                    }
                                }
                            }
                            SimulationEventWorldEntityType::Projectile { .. } | SimulationEventWorldEntityType::Pickup { .. }  |  SimulationEventWorldEntityType::Laser { .. } => {
                                // ignore
                            }
                        },
                        SimulationWorldEvent::Notification(_) => {
                            // ignore
                        }
                    }
                }));
        }

        /// returns true, if match needs a restart
        #[must_use]
        pub fn tick(&mut self, world: &mut GameWorld) -> bool {
            self.handle_events(world);

            if let MatchState::GameOver { new_game_in, .. } = &mut self.game_match.state {
                if new_game_in.tick().unwrap_or_default() {
                    self.game_match.state = MatchState::Running {
                        round_ticks_passed: Default::default(),
                    };
                    world.characters.values_mut().for_each(|char| {
                        char.drop_mode(DropMode::NoEvents);
                        char.die(None, GameWorldActionKillWeapon::World);
                    });
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
    }
}
