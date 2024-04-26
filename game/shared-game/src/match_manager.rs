/// everything related to a single match/round/race-run
pub mod match_manager {
    use game_interface::{
        interface::GameType,
        types::game::{GameEntityId, GameTickCooldown, GameTickType},
    };
    use hiarc::{hi_closure, Hiarc};
    use serde::{Deserialize, Serialize};

    use crate::{
        entities::character::character::Character,
        events::events::CharacterEvent,
        simulation_pipe::simulation_pipe::{
            SimulationEventWorldEntity, SimulationStageEvents, SimulationWorldEvent,
        },
        state::state::TICKS_PER_SECOND,
        types::types::{GameOptions, GameTeam},
        world::world::GameWorld,
    };

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
    pub enum MatchWinner {
        Player(GameEntityId),
        Team(GameTeam),
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
    pub enum MatchState {
        Running,
        Paused,
        GameOver {
            winner: MatchWinner,
            new_game_in: GameTickCooldown,
        },
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
    pub enum MatchType {
        Solo,
        Team { scores: [i64; 2] },
    }

    /// the snappable part of the match manager
    #[derive(Debug, Hiarc)]
    pub struct Match {
        pub(crate) ty: MatchType,
        pub(crate) state: MatchState,
    }

    impl Match {
        // TODO: sudden death
        pub fn win_check(&mut self, characters_with_score_change: &[&Character]) {
            // TODO: random 4 seconds
            const TICKS_UNTIL_NEW_GAME: GameTickType = TICKS_PER_SECOND * 4;
            match self.ty {
                MatchType::Solo => {
                    // check if the character has hit a specific score
                    let char = characters_with_score_change
                        .iter()
                        .find(|char| char.core.score >= i64::MAX /* TODO */);
                    if let Some(char) = char {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Player(char.base.game_element_id),
                            new_game_in: TICKS_UNTIL_NEW_GAME.into(),
                        }
                    }
                }
                MatchType::Team { scores } => {
                    // check if team has hit a specific score
                    if scores[0] >= 5
                    /* TODO */
                    {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Team(GameTeam::Red),
                            new_game_in: TICKS_UNTIL_NEW_GAME.into(),
                        };
                    } else if scores[1] >= 5
                    /* TODO */
                    {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Team(GameTeam::Blue),
                            new_game_in: TICKS_UNTIL_NEW_GAME.into(),
                        };
                    }
                }
            }
        }
    }

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
            self.simulation_events
                .for_each(hi_closure!([game_match: &mut Match, world: &mut GameWorld], |ev: &SimulationWorldEvent| -> () {
                    match ev {
                        SimulationWorldEvent::Entity(ev) => match ev {
                            SimulationEventWorldEntity::Character { ev, .. } => match ev {
                                CharacterEvent::Despawn { killer_id } => {
                                    if let Some(killer_id) = killer_id {
                                        if let Some(char) = world.characters.get_mut(killer_id) {
                                            char.core.score += 1;
                                            if let MatchType::Team { scores } = &mut game_match.ty {
                                                scores[char.core.team.unwrap() as usize] += 1;
                                            }
                                            game_match.win_check(&[char]);
                                        }
                                    }
                                }
                                _ => {}
                            },
                            SimulationEventWorldEntity::Projectile { .. } | SimulationEventWorldEntity::Pickup { .. }  | SimulationEventWorldEntity::Flag { .. } | SimulationEventWorldEntity::Laser { .. } => {
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
            self.handle_events_impl(world);

            if let MatchState::GameOver { new_game_in, .. } = &mut self.game_match.state {
                if new_game_in.tick().unwrap_or_default() {
                    self.game_match.state = MatchState::Running;
                    world.characters.iter_mut().for_each(|(id, char)| {
                        char.die(None);
                        for ev in char.entity_events.drain(..) {
                            self.simulation_events.push(SimulationWorldEvent::Entity(
                                SimulationEventWorldEntity::Character { player_id: *id, ev },
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
