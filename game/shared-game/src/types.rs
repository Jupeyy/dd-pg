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
    }

    #[derive(Debug, Hiarc, Clone, Copy)]
    pub struct GameOptions(GameOptionsInner);

    impl GameOptions {
        pub fn new(ty: GameType) -> Self {
            Self(GameOptionsInner { ty })
        }
    }

    impl Deref for GameOptions {
        type Target = GameOptionsInner;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}
