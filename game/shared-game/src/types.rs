pub mod types {
    use std::ops::Deref;

    use hiarc::Hiarc;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Hiarc, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
    pub enum GameTeam {
        Red,
        Blue,
    }

    #[derive(Debug, Hiarc, Clone, Copy, Default, Serialize, Deserialize)]
    pub enum GameType {
        #[default]
        Solo,
        Team,
    }

    #[derive(Debug, Hiarc, Clone, Copy)]
    pub struct GameOptionsInner {
        pub ty: GameType,
        pub score_limit: u64,
    }

    #[derive(Debug, Hiarc, Clone, Copy)]
    pub struct GameOptions(GameOptionsInner);

    impl GameOptions {
        pub fn new(ty: GameType, score_limit: u64) -> Self {
            Self(GameOptionsInner { ty, score_limit })
        }
    }

    impl Deref for GameOptions {
        type Target = GameOptionsInner;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}
