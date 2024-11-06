pub mod types {
    use std::{ops::Deref, rc::Rc};

    use hiarc::Hiarc;
    use serde::{Deserialize, Serialize};

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
        pub friendly_fire: bool,
        pub laser_hit_self: bool,
    }

    #[derive(Debug, Hiarc, Clone)]
    pub struct GameOptions(Rc<GameOptionsInner>);

    impl GameOptions {
        pub fn new(
            ty: GameType,
            score_limit: u64,
            friendly_fire: bool,
            laser_hit_self: bool,
        ) -> Self {
            Self(Rc::new(GameOptionsInner {
                ty,
                score_limit,
                friendly_fire,
                laser_hit_self,
            }))
        }
    }

    impl Deref for GameOptions {
        type Target = GameOptionsInner;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}
