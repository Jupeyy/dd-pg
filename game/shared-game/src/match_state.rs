/// everything related to a single match/round/race-run
pub mod match_state {
    use game_interface::types::{
        game::{GameEntityId, GameTickCooldown, GameTickType},
        render::game::game_match::MatchSide,
    };
    use hiarc::Hiarc;
    use serde::{Deserialize, Serialize};

    use crate::{
        entities::character::character::Character, state::state::TICKS_PER_SECOND,
        types::types::GameOptions,
    };

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
    pub enum MatchWinner {
        Player(GameEntityId),
        Side(MatchSide),
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
    pub enum MatchState {
        Running {
            /// How long the game round is running.
            round_ticks_passed: GameTickType,
        },
        Paused {
            /// How long the game round is running.
            round_ticks_passed: GameTickType,
        },
        GameOver {
            winner: MatchWinner,
            new_game_in: GameTickCooldown,
            /// How long the game round is running.
            round_ticks_passed: GameTickType,
        },
    }

    impl MatchState {
        pub fn passed_ticks(&self) -> GameTickType {
            match self {
                MatchState::Running { round_ticks_passed } => *round_ticks_passed,
                MatchState::Paused { round_ticks_passed } => *round_ticks_passed,
                MatchState::GameOver {
                    round_ticks_passed, ..
                } => *round_ticks_passed,
            }
        }
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize, Clone, Copy)]
    pub enum MatchType {
        Solo,
        Sided { scores: [i64; 2] },
    }

    /// the snappable part of the match manager
    #[derive(Debug, Hiarc)]
    pub struct Match {
        pub ty: MatchType,
        pub state: MatchState,
    }

    impl Match {
        // TODO: sudden death
        pub fn win_check(
            &mut self,
            game_options: &GameOptions,
            characters_with_score_change: &[&Character],
        ) {
            let cur_tick = self.state.passed_ticks();
            // TODO: random 4 seconds
            const TICKS_UNTIL_NEW_GAME: GameTickType = TICKS_PER_SECOND * 4;
            match self.ty {
                MatchType::Solo => {
                    // check if the character has hit a specific score
                    let char = characters_with_score_change.iter().find(|char| {
                        char.core.score >= 0 && char.core.score as u64 >= game_options.score_limit
                    });
                    if let Some(char) = char {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Player(char.base.game_element_id),
                            new_game_in: TICKS_UNTIL_NEW_GAME.into(),
                            round_ticks_passed: cur_tick,
                        }
                    }
                }
                MatchType::Sided { scores } => {
                    // check if team has hit a specific score
                    if scores[0] >= 0 && scores[0] as u64 >= game_options.score_limit {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Side(MatchSide::Red),
                            new_game_in: TICKS_UNTIL_NEW_GAME.into(),
                            round_ticks_passed: cur_tick,
                        };
                    } else if scores[1] >= 0 && scores[1] as u64 >= game_options.score_limit {
                        // TODO:
                        self.state = MatchState::GameOver {
                            winner: MatchWinner::Side(MatchSide::Blue),
                            new_game_in: TICKS_UNTIL_NEW_GAME.into(),
                            round_ticks_passed: cur_tick,
                        };
                    }
                }
            }
        }

        pub fn tick(&mut self) {
            match &mut self.state {
                MatchState::Running { round_ticks_passed } => {
                    *round_ticks_passed += 1;
                }
                MatchState::Paused { .. } => {
                    // nothing to do
                }
                MatchState::GameOver { .. } => {
                    // nothing to do
                }
            }
        }
    }
}
