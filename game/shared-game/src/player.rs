pub mod player {
    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::vector::{dvec2, vec2};
    use serde::{Deserialize, Serialize};

    use shared_base::{
        game_types::TGameElementID,
        network::messages::{MsgObjPlayerInfo, MsgObjPlayerInput},
        types::GameTickType,
    };

    use crate::{
        entities::character_core::character_core::HookState,
        weapons::definitions::weapon_def::WeaponType,
    };

    use super::super::simulation_pipe::simulation_pipe::SimulationPlayerInput;

    #[derive(Serialize, Deserialize, Clone, Encode, Decode, Default)]
    pub struct PlayerCharacterInfo {
        pub character_id: TGameElementID,
        pub stage_id: TGameElementID,
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
    pub struct PlayerInput {
        pub inp: MsgObjPlayerInput,
        pub version: u64,
    }

    #[derive(Clone)]
    pub struct Player {
        pub player_info: MsgObjPlayerInfo,
        pub input: PlayerInput,
        pub id: TGameElementID,

        pub character_info: PlayerCharacterInfo,
        pub version: u64,

        /// the game start tick
        /// It's based on the `cur_monotonic_tick` from the `State`
        /// and represents the tick offset which the player sees the game
        /// E.g. it could be the race start tick, the current game round start tick
        pub game_start_tick: GameTickType,

        /// the animation start tick is for client side rendering
        /// and thus only send to local players (or spec'ed players) in the snapshot
        pub animation_start_tick: GameTickType,
    }

    impl Player {
        pub fn new(
            player_info: MsgObjPlayerInfo,
            id: &TGameElementID,
            character_info: PlayerCharacterInfo,
            version: u64,
            game_start_tick: GameTickType,
            animation_start_tick: GameTickType,
        ) -> Self {
            Self {
                player_info: player_info,
                input: PlayerInput::default(),

                character_info: character_info,

                id: id.clone(),
                version,
                game_start_tick,
                animation_start_tick,
            }
        }
    }

    pub type Players = LinkedHashMap<TGameElementID, Player>;

    impl SimulationPlayerInput for Players {
        fn get_input(&self, player_id: &TGameElementID) -> Option<&MsgObjPlayerInput> {
            let res = self.get(player_id);
            match res {
                Some(player) => Some(&player.input.inp),
                None => None,
            }
        }
    }

    #[derive(Copy, Clone, Serialize, Deserialize, Encode, Decode, Default)]
    pub enum NoCharPlayerType {
        Spectator,
        Dead,

        #[default]
        Unknown,
    }

    #[derive(Clone)]
    pub struct NoCharPlayer {
        pub player_info: MsgObjPlayerInfo,
        pub id: TGameElementID,
        pub version: u64,
        pub no_char_type: NoCharPlayerType,

        // mostly interesting for server
        pub respawns_at_tick: GameTickType,
        pub last_stage_id: TGameElementID,
    }

    impl NoCharPlayer {
        pub fn new(
            player_info: &MsgObjPlayerInfo,
            id: &TGameElementID,
            version: u64,
            no_char_type: NoCharPlayerType,
        ) -> Self {
            Self {
                player_info: player_info.clone(),
                id: id.clone(),
                version,
                no_char_type,

                respawns_at_tick: GameTickType::default(),
                last_stage_id: TGameElementID::default(),
            }
        }
    }

    pub type NoCharPlayers = LinkedHashMap<TGameElementID, NoCharPlayer>;

    #[derive(Clone)]
    pub struct UnknownPlayer {
        pub player_info: MsgObjPlayerInfo,
        pub id: TGameElementID,
        pub version: u64,
    }

    impl UnknownPlayer {
        pub fn new(player_info: &MsgObjPlayerInfo, id: &TGameElementID, version: u64) -> Self {
            Self {
                player_info: player_info.clone(),
                id: id.clone(),
                version,
            }
        }
    }

    #[derive(Clone)]
    pub struct PlayerRemoveInfo {
        pub respawns_at_tick: Option<GameTickType>,
        pub last_stage_id: TGameElementID,
        pub no_char_type: NoCharPlayerType,
        pub pos: vec2,
    }

    #[derive(Clone, Serialize, Deserialize, Encode, Decode)]
    pub struct PlayerRenderInfo {
        pub lerped_pos: vec2,
        pub lerped_vel: vec2,
        pub lerped_hook_pos: vec2,
        pub hook_state: HookState,
        pub cursor_pos: dvec2,
        pub move_dir: i32,
        pub cur_weapon: WeaponType,
        pub recoil_start_tick: GameTickType,
        // TODO: add fields to make this more flexible for modding
    }
}
