pub mod player {
    use std::marker::PhantomData;

    use base::hash::Hash;
    use game_interface::types::character_info::NetworkCharacterInfo;
    use game_interface::types::game::{GameEntityId, GameTickCooldown};
    use game_interface::types::input::CharacterInput;
    use hashlink::LinkedHashMap;
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use hiarc::{HiFnMut, HiFnOnce};
    use pool::datatypes::{PoolLinkedHashMap, PoolVec};
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

    #[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
    pub struct PlayerInfo {
        pub player_info: NetworkCharacterInfo,
        pub version: u64,

        #[serde(skip)]
        pub unique_identifier: Option<Hash>,
        pub is_dummy: bool,
    }

    pub type Player = PlayerCharacterInfo;

    /// A slim wrapper around the character info around the player.
    /// A player contains no additional information, instead the player info
    /// is stored in the character info.
    /// This is different compared to a [`NoCharPlayer`] which does contain the
    /// player info and other stuff.
    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
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
        pub(crate) fn contains_key(&self, id: &GameEntityId) -> bool {
            self.players.contains_key(id)
        }
        pub(crate) fn to_back(&mut self, id: &GameEntityId) {
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
        pub(crate) fn len(&self) -> usize {
            self.players.len()
        }
    }

    #[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize, Default)]
    pub enum NoCharPlayerType {
        Spectator,
        Dead {
            respawn_in_ticks: GameTickCooldown,
        },

        #[default]
        Unknown,
    }

    #[derive(Debug, Hiarc, Clone)]
    pub struct NoCharPlayer {
        pub player_info: PlayerInfo,
        pub player_input: CharacterInput,
        pub id: GameEntityId,
        pub no_char_type: NoCharPlayerType,

        // mostly interesting for server
        pub last_stage_id: Option<GameEntityId>,
    }

    impl NoCharPlayer {
        pub fn new(
            player_info: PlayerInfo,
            player_input: CharacterInput,
            id: &GameEntityId,
            no_char_type: NoCharPlayerType,
        ) -> Self {
            Self {
                player_info,
                player_input,
                id: id.clone(),
                no_char_type,

                last_stage_id: Default::default(),
            }
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
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
            self.players.get(id).cloned()
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
        pub(crate) fn to_back(&mut self, id: &GameEntityId) {
            self.players.to_back(id);
        }
        pub(crate) fn pooled_clone_into(&self, copy_pool: &mut PoolVec<NoCharPlayer>) {
            copy_pool.extend(self.players.iter().map(|(_, player)| player.clone()));
        }
        pub(crate) fn len(&self) -> usize {
            self.players.len()
        }
        pub(crate) fn retain_with_order<F>(&mut self, mut f: F)
        where
            for<'a> F: HiFnMut<(&'a GameEntityId, &'a mut NoCharPlayer), bool>,
        {
            self.players
                .retain_with_order(|id, player| f.call_mut((id, player)))
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

    #[derive(Debug, Clone)]
    pub struct UnknownPlayer {
        pub player_info: PlayerInfo,
        pub id: GameEntityId,
    }

    impl UnknownPlayer {
        pub fn new(player_info: PlayerInfo, id: &GameEntityId) -> Self {
            Self {
                player_info,
                id: id.clone(),
            }
        }
    }
    pub type UknPlayers = LinkedHashMap<GameEntityId, UnknownPlayer>;
}
