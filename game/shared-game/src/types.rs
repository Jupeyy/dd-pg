pub mod types {
    use std::ops::Deref;

    use bincode::{Decode, Encode};
    use pool::datatypes::PoolString;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct PlayerChatInfo {
        pub skin_name: PoolString,
        pub player_name: PoolString,
        // TODO: add fields to make this more flexible for modding
    }

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub struct PlayerScoreboardInfo {
        pub skin_name: PoolString,
        pub player_name: PoolString,
        pub clan_name: PoolString,
        pub flag_name: PoolString,
        pub score: i64,
        pub ping: u64,
        // TODO: add fields to make this more flexible for modding
    }

    pub type PlayerScoreboardSpectatorInfo = PlayerScoreboardInfo;

    #[derive(Serialize, Deserialize, Encode, Decode)]
    pub enum ScoreboardGameType {
        /// team = vanilla team
        TeamPlay {
            red_players: Vec<PlayerScoreboardInfo>,
            blue_players: Vec<PlayerScoreboardInfo>,
            spectator_players: Vec<PlayerScoreboardSpectatorInfo>,
        },
        SoloPlay {
            players: Vec<PlayerScoreboardInfo>,
            spectator_players: Vec<PlayerScoreboardSpectatorInfo>,
        },
    }

    #[derive(
        Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize, Encode, Decode,
    )]
    pub enum GameTeam {
        Red,
        Blue,
    }

    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Encode, Decode)]
    pub enum GameType {
        #[default]
        Solo,
        Team,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct GameOptionsInner {
        pub ty: GameType,
    }

    #[derive(Debug, Clone, Copy)]
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
