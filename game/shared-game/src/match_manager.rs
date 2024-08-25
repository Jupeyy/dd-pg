/// everything related to a single match/round/race-run
pub mod match_manager {
    use game_interface::{events::GameWorldActionKillWeapon, types::game::GameEntityId};
    use hiarc::{hi_closure, Hiarc};

    use crate::{
        ctf_controller::ctf_controller::CtfController,
        events::events::{CharacterEvent, FlagEvent},
        match_state::match_state::{Match, MatchState, MatchType},
        simulation_pipe::simulation_pipe::{
            SimulationEventWorldEntity, SimulationEventWorldEntityType, SimulationStageEvents,
            SimulationWorldEvent,
        },
        types::types::{GameOptions, GameType},
        world::world::GameWorld,
    };

    #[derive(Debug, Hiarc)]
    pub struct MatchManager {
        pub(crate) game_options: GameOptions,
        simulation_events: SimulationStageEvents,

        pub(crate) game_match: Match,

        stage_id: GameEntityId,
    }

    impl MatchManager {
        pub fn new(
            stage_id: GameEntityId,
            game_options: GameOptions,
            simulation_events: &SimulationStageEvents,
        ) -> Self {
            Self {
                game_match: Match {
                    ty: match game_options.ty {
                        GameType::Solo => MatchType::Solo,
                        GameType::Team => MatchType::Team {
                            scores: Default::default(),
                        },
                    },
                    state: MatchState::Running,
                },
                game_options,
                simulation_events: simulation_events.clone(),
                stage_id,
            }
        }

        fn handle_events_impl(&mut self, world: &mut GameWorld) {
            let game_match = &mut self.game_match;
            let game_options = &self.game_options;
            self.simulation_events
                .for_each(hi_closure!([game_match: &mut Match, game_options: &GameOptions, world: &mut GameWorld], |ev: &SimulationWorldEvent| -> () {
                    match ev {
                        SimulationWorldEvent::Entity(entity_ev) => match &entity_ev.ev {
                            SimulationEventWorldEntityType::Character { ev, .. } => if let CharacterEvent::Despawn { killer_id, .. } = ev {
                                if let Some(char) = killer_id.and_then(|killer_id| world.characters.get_mut(&killer_id)) {
                                    char.core.score += 1;
                                    if let (MatchType::Team { scores }, Some(team)) = (&mut game_match.ty, char.core.team) {
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
                                            if let (MatchType::Team { scores }, Some(team)) = (&mut game_match.ty, char.core.team) {
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
                        SimulationWorldEvent::Global(_) => {
                            // ignore
                        }
                    }
                }));
        }

        /// returns true, if match needs a restart
        #[must_use]
        pub fn handle_events(&mut self, world: &mut GameWorld) -> bool {
            if matches!(self.game_match.ty, MatchType::Team { .. }) {
                CtfController::tick(world);
            }

            self.handle_events_impl(world);

            if let MatchState::GameOver { new_game_in, .. } = &mut self.game_match.state {
                if new_game_in.tick().unwrap_or_default() {
                    self.game_match.state = MatchState::Running;
                    world.characters.iter_mut().for_each(|(id, char)| {
                        char.die(None, GameWorldActionKillWeapon::World);
                        for ev in char.entity_events.drain(..) {
                            self.simulation_events.push(SimulationWorldEvent::Entity(
                                SimulationEventWorldEntity {
                                    ev: SimulationEventWorldEntityType::Character { ev },
                                    owner_id: Some(*id),
                                },
                            ));
                        }
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
