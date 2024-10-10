pub mod player {
    use std::marker::PhantomData;

    use game_interface::types::character_info::NetworkCharacterInfo;
    use game_interface::types::game::{GameEntityId, GameTickCooldown};
    use game_interface::types::input::CharacterInput;
    use game_interface::types::network_stats::PlayerNetworkStats;
    use game_interface::types::player_info::PlayerUniqueId;
    use game_interface::types::render::game::game_match::MatchSide;
    use hashlink::LinkedHashMap;
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use hiarc::{HiFnMut, HiFnOnce};
    use math::math::vector::vec2;
    use pool::datatypes::{PoolLinkedHashMap, PoolVec};
    use pool::rc::PoolRc;
    use serde::{Deserialize, Serialize};

    /// This purposely does not implement [`Clone`].
    /// Instead the user should always query the current character info.
    /// (it might have been changed by other logic as a side effect)
    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub struct PlayerCharacterInfo {
        pub(in super::super::super::character) stage_id: GameEntityId,
    }

    impl PlayerCharacterInfo {
        pub fn stage_id(&self) -> GameEntityId {
            self.stage_id
        }
    }

    #[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
    pub struct PlayerInfo {
        pub player_info: PoolRc<NetworkCharacterInfo>,
        pub version: u64,

        pub unique_identifier: PlayerUniqueId,
        pub player_index: usize,
        pub is_dummy: bool,
    }

    pub type Player = PlayerCharacterInfo;

    /// A slim wrapper around the character info around the player.
    ///
    /// A player contains no additional information, instead the player info
    /// is stored in the character info.
    /// This is different compared to a [`NoCharPlayer`] which does contain the
    /// player info and other stuff.
    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc, Default)]
    pub struct Players {
        players: LinkedHashMap<GameEntityId, Player>,
    }

    #[hiarc_safer_rc_refcell]
    impl Players {
        pub fn new() -> Self {
            Self {
                players: Default::default(),
            }
        }

        pub fn player(&self, id: &GameEntityId) -> Option<Player> {
            let player = self.players.get(id)?;
            Some(Player {
                stage_id: player.stage_id,
            })
        }

        pub(in super::super::super::character) fn insert(
            &mut self,
            id: GameEntityId,
            player: Player,
        ) {
            self.players.insert(id, player);
        }
        pub(in super::super::super::character) fn remove(&mut self, id: &GameEntityId) {
            self.players.remove(id);
        }
        pub(crate) fn move_to_back(&mut self, id: &GameEntityId) {
            self.players.to_back(id);
        }
        pub(crate) fn pooled_clone_into(&self, copy_pool: &mut PoolVec<(GameEntityId, Player)>) {
            copy_pool.extend(self.players.iter().map(|(id, player)| {
                (
                    *id,
                    Player {
                        stage_id: player.stage_id,
                    },
                )
            }));
        }
    }

    #[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
    pub enum NoCharPlayerType {
        Spectator,
        Dead {
            respawn_in_ticks: GameTickCooldown,
            side: Option<MatchSide>,
            score: i64,
            // mostly interesting for server
            stage_id: GameEntityId,
            died_at_pos: vec2,
        },
    }

    #[derive(Debug, Hiarc)]
    pub struct NoCharPlayer {
        pub player_info: PlayerInfo,
        pub player_input: CharacterInput,
        pub id: GameEntityId,
        pub no_char_type: NoCharPlayerType,

        pub network_stats: PlayerNetworkStats,
    }

    impl NoCharPlayer {
        pub fn new(
            player_info: PlayerInfo,
            player_input: CharacterInput,
            id: &GameEntityId,
            no_char_type: NoCharPlayerType,
            network_stats: PlayerNetworkStats,
        ) -> Self {
            Self {
                player_info,
                player_input,
                id: *id,
                no_char_type,

                network_stats,
            }
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc, Default)]
    pub struct NoCharPlayers {
        players: LinkedHashMap<GameEntityId, NoCharPlayer>,

        // force higher hierarchy val
        _passed: PhantomData<PoolLinkedHashMap<GameEntityId, NoCharPlayer>>,
    }

    #[hiarc_safer_rc_refcell]
    impl NoCharPlayers {
        pub fn new() -> Self {
            Self {
                players: Default::default(),

                _passed: Default::default(),
            }
        }

        pub fn player(&self, id: &GameEntityId) -> Option<NoCharPlayer> {
            self.players.get(id).map(|player| {
                NoCharPlayer::new(
                    player.player_info.clone(),
                    player.player_input,
                    id,
                    player.no_char_type,
                    player.network_stats,
                )
            })
        }
        pub fn contains_key(&self, id: &GameEntityId) -> bool {
            self.players.get(id).is_some()
        }

        pub fn insert(&mut self, id: GameEntityId, player: NoCharPlayer) {
            self.players.insert(id, player);
        }
        pub fn remove(&mut self, id: &GameEntityId) -> Option<NoCharPlayer> {
            self.players.remove(id)
        }
        pub(crate) fn move_to_back(&mut self, id: &GameEntityId) {
            self.players.to_back(id);
        }
        pub(crate) fn pooled_clone_into(
            &self,
            copy_pool: &mut PoolLinkedHashMap<GameEntityId, NoCharPlayer>,
        ) {
            for (id, player) in self.players.iter() {
                copy_pool.insert(*id, {
                    NoCharPlayer::new(
                        player.player_info.clone(),
                        player.player_input,
                        id,
                        player.no_char_type,
                        player.network_stats,
                    )
                });
            }
        }
        pub(crate) fn count_players_per_side(&self, stage_id: GameEntityId) -> (usize, usize) {
            let mut red = 0;
            let mut blue = 0;
            self.players.iter().for_each(|(_, char)| {
                match &char.no_char_type {
                    NoCharPlayerType::Dead {
                        side: Some(side),
                        stage_id: dead_stage_id,
                        ..
                    } => {
                        if stage_id == *dead_stage_id {
                            match side {
                                MatchSide::Blue => blue += 1,
                                MatchSide::Red => red += 1,
                            }
                        }
                    }
                    _ => {
                        // ignore
                    }
                }
            });
            (red, blue)
        }
        pub(crate) fn any_player_in(&self, stage_id: GameEntityId) -> bool {
            self.players.values().any(|char| match &char.no_char_type {
                NoCharPlayerType::Dead {
                    stage_id: dead_stage_id,
                    ..
                } => stage_id == *dead_stage_id,
                _ => false,
            })
        }
        pub(crate) fn retain_with_order<F>(&mut self, mut f: F)
        where
            for<'a> F: HiFnMut<(&'a GameEntityId, &'a mut NoCharPlayer), bool>,
        {
            self.players
                .retain_with_order(|id, player| f.call_mut((id, player)))
        }
        pub(crate) fn for_each<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<(&'a GameEntityId, &'a NoCharPlayer), ()>,
        {
            self.players
                .iter()
                .for_each(|(id, player)| f.call_mut((id, player)))
        }
        /// handle a no char player
        /// returns false if the player did not exist, else true
        pub(crate) fn handle_mut<F>(&mut self, id: &GameEntityId, f: F) -> bool
        where
            for<'a> F: HiFnOnce<&'a mut NoCharPlayer, ()>,
        {
            match self.players.get_mut(id) {
                Some(player) => {
                    f.call_once(player);
                    true
                }
                None => false,
            }
        }
    }
}
