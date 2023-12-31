/// everything related to a single match/round/race-run
pub mod match_manager {
    use bincode::{Decode, Encode};
    use serde::{Deserialize, Serialize};
    use shared_base::{game_types::TGameElementID, types::GameTickType};

    use crate::{
        entities::character::character::Character,
        events::events::CharacterEvent,
        simulation_pipe::simulation_pipe::{SimulationEvent, SimulationEventsWorld},
        state::state::TICKS_PER_SECOND,
        types::types::{GameOptions, GameTeam, GameType},
        world::world::GameWorld,
    };

    #[derive(Debug, Serialize, Deserialize, Encode, Decode, Clone, Copy)]
    pub enum MatchWinner {
        Player(TGameElementID),
        Team(GameTeam),
    }

    #[derive(Debug, Serialize, Deserialize, Encode, Decode, Clone, Copy)]
    pub enum MatchState {
        Running,
        Paused,
        GameOver {
            winner: MatchWinner,
            tick: GameTickType,
        },
    }

    #[derive(Debug, Serialize, Deserialize, Encode, Decode, Clone, Copy)]
    pub enum MatchType {
        Solo,
        Team { scores: [i64; 2] },
    }

    pub struct MatchManager {
        pub(crate) ty: MatchType,
        pub(crate) game_options: GameOptions,
        pub(crate) state: MatchState,

        stage_id: TGameElementID,
    }

    impl MatchManager {
        pub fn new(stage_id: TGameElementID, game_options: GameOptions) -> Self {
            Self {
                ty: match game_options.ty {
                    GameType::Solo => MatchType::Solo,
                    GameType::Team => MatchType::Team {
                        scores: Default::default(),
                    },
                },
                game_options,
                state: MatchState::Running,
                stage_id,
            }
        }

        // TODO: sudden death
        pub fn win_check(
            &mut self,
            cur_tick: GameTickType,
            characters_with_score_change: &[&Character],
        ) {
            match self.ty {
                MatchType::Solo => {
                    // check if the character has hit a specific score
                    let char = characters_with_score_change
                        .iter()
                        .find(|char| char.core.score >= 5 /* TODO */);
                    if let Some(char) = char {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Player(char.base.game_element_id),
                            tick: cur_tick,
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
                            tick: cur_tick,
                        };
                    } else if scores[1] >= 5
                    /* TODO */
                    {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Team(GameTeam::Blue),
                            tick: cur_tick,
                        };
                    }
                }
            }
        }

        /// returns true, if match needs a restart
        #[must_use]
        pub fn handle_events(
            &mut self,
            cur_tick: GameTickType,
            world: &mut GameWorld,
            events: &mut Vec<SimulationEvent>,
        ) -> bool {
            for ev in events.iter() {
                match ev {
                    SimulationEvent::World { ev, .. } => match ev {
                        SimulationEventsWorld::Character { ev, .. } => match ev {
                            CharacterEvent::Despawn { killer_id, .. } => {
                                if let Some(killer_id) = killer_id {
                                    if let Some(char) = world.characters.get_mut(killer_id) {
                                        char.core.score += 1;
                                        if let MatchType::Team { scores } = &mut self.ty {
                                            scores[char.core.team.unwrap() as usize] += 1;
                                        }
                                        self.win_check(cur_tick, &[char]);
                                    }
                                }
                            }
                            _ => {}
                        },
                    },
                }
            }

            // TODO: random 4 seconds
            const TICKS_WHEN_GAME_OVER: GameTickType = TICKS_PER_SECOND * 4;
            if let MatchState::GameOver { tick, .. } = self.state {
                if cur_tick > tick + TICKS_WHEN_GAME_OVER {
                    self.state = MatchState::Running;
                    world.characters.iter_mut().for_each(|(id, char)| {
                        char.die(cur_tick, None);
                        for ev in char.entity_events.drain(..) {
                            events.push(SimulationEvent::World {
                                stage_id: self.stage_id,
                                ev: SimulationEventsWorld::Character { player_id: *id, ev },
                            });
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
