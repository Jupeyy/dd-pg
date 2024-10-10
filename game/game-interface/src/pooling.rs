use std::borrow::Cow;

use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use pool::datatypes::StringPool;
use pool::mt_datatypes::StringPool as MtStringPool;
use pool::mt_pool::Pool as MtPool;
use pool::pool::Pool;

use crate::events::{EventId, GameWorldEvent, GameWorldEvents};
use crate::types::character_info::NetworkCharacterInfo;
use crate::types::game::GameEntityId;
use crate::types::render::character::{
    CharacterBuff, CharacterBuffInfo, CharacterDebuff, CharacterDebuffInfo, CharacterInfo,
    CharacterRenderInfo,
};
use crate::types::render::flag::FlagRenderInfo;
use crate::types::render::laser::LaserRenderInfo;
use crate::types::render::pickup::PickupRenderInfo;
use crate::types::render::projectiles::ProjectileRenderInfo;
use crate::types::render::scoreboard::{
    ScoreboardCharacterInfo, ScoreboardPlayerSpectatorInfo, ScoreboardStageInfo,
};
use crate::types::render::stage::StageRenderInfo;

/// Make your life easier by simply using all required pools for the interface
#[derive(Debug, Hiarc)]
pub struct GamePooling {
    pub string_pool: StringPool,
    pub mt_string_pool: MtStringPool,
    pub stage_render_info: Pool<LinkedHashMap<GameEntityId, StageRenderInfo>>,
    pub character_render_info_pool: Pool<LinkedHashMap<GameEntityId, CharacterRenderInfo>>,
    pub character_info_pool: Pool<LinkedHashMap<GameEntityId, CharacterInfo>>,
    pub entity_id_pool: MtPool<Vec<GameEntityId>>,
    pub projectile_render_info_pool: Pool<LinkedHashMap<GameEntityId, ProjectileRenderInfo>>,
    pub flag_render_info_pool: Pool<LinkedHashMap<GameEntityId, FlagRenderInfo>>,
    pub laser_render_info_pool: Pool<LinkedHashMap<GameEntityId, LaserRenderInfo>>,
    pub pickup_render_info_pool: Pool<LinkedHashMap<GameEntityId, PickupRenderInfo>>,
    pub stage_scoreboard_pool: Pool<LinkedHashMap<GameEntityId, ScoreboardStageInfo>>,
    pub character_scoreboard_pool: Pool<Vec<ScoreboardCharacterInfo>>,
    pub player_spectator_scoreboard_pool: Pool<Vec<ScoreboardPlayerSpectatorInfo>>,
    pub character_infos_pool_short: Pool<Vec<(GameEntityId, NetworkCharacterInfo)>>,
    pub character_buffs: Pool<LinkedHashMap<CharacterBuff, CharacterBuffInfo>>,
    pub character_debuffs: Pool<LinkedHashMap<CharacterDebuff, CharacterDebuffInfo>>,
    pub snapshot_pool: MtPool<Cow<'static, [u8]>>,
    pub worlds_events_pool: MtPool<LinkedHashMap<GameEntityId, GameWorldEvents>>,
    pub world_events_pool: MtPool<LinkedHashMap<EventId, GameWorldEvent>>,
}

impl GamePooling {
    pub fn new(hint_max_characters: Option<usize>) -> Self {
        let hint_max_characters = hint_max_characters.unwrap_or(64);
        Self {
            string_pool: StringPool::with_capacity(64),
            mt_string_pool: MtStringPool::with_capacity(64),
            stage_render_info: Pool::with_capacity(2),
            character_render_info_pool: Pool::with_capacity(hint_max_characters),
            character_info_pool: Pool::with_capacity(64),
            entity_id_pool: MtPool::with_capacity(64),
            projectile_render_info_pool: Pool::with_capacity(64),
            flag_render_info_pool: Pool::with_capacity(64),
            laser_render_info_pool: Pool::with_capacity(64),
            pickup_render_info_pool: Pool::with_capacity(64),
            stage_scoreboard_pool: Pool::with_capacity(64),
            character_scoreboard_pool: Pool::with_capacity(64),
            player_spectator_scoreboard_pool: Pool::with_capacity(64),
            character_infos_pool_short: Pool::with_capacity(2),
            character_buffs: Pool::with_capacity(64),
            character_debuffs: Pool::with_capacity(64),
            snapshot_pool: MtPool::with_capacity(2),
            worlds_events_pool: MtPool::with_capacity(2),
            world_events_pool: MtPool::with_capacity(64),
        }
    }
}
