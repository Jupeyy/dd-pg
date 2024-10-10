use game_interface::types::{character_info::NetworkSkinInfo, resource_key::ResourceKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoritePlayer {
    pub name: String,
    pub clan: String,
    pub skin: ResourceKey,
    pub skin_info: NetworkSkinInfo,
    pub flag: String,
}

pub type FavoritePlayers = Vec<FavoritePlayer>;
