/// everything related to a single match/round/race-run
pub mod match_state {
    use game_interface::types::game::{GameEntityId, GameTickCooldown, GameTickType};
    use hiarc::Hiarc;
    use serde::{Deserialize, Serialize};

    use crate::{
        entities::character::character::Character, state::state::TICKS_PER_SECOND,
        types::types::GameTeam,
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
        pub ty: MatchType,
        pub state: MatchState,
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
}
