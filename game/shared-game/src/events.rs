pub mod events {
    use bincode::{Decode, Encode};
    use math::math::vector::vec2;
    use shared_base::{
        game_types::TGameElementID, network::messages::WeaponType, types::GameTickType,
    };

    use crate::player::player::PlayerInfo;

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum EntityEvent {
        Sound {
            pos: vec2,
            name: String, // TODO: stack string? pool string?
        },
        Explosion {
            // TODO:
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum ProjectileEvent {
        Entity(EntityEvent),
        Despawn {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum LaserEvent {
        Entity(EntityEvent),
        Despawn {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum PickupEvent {
        Entity(EntityEvent),
        Despawn {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum FlagEvent {
        Entity(EntityEvent),
        Despawn {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum CharacterEvent {
        Entity(EntityEvent),
        Despawn {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
            player_info: PlayerInfo,
            killer_id: Option<TGameElementID>,
        },
        Projectile {
            pos: vec2,
            dir: vec2,
            ty: WeaponType,
        },
        Laser {
            pos: vec2,
            dir: vec2,
        },
        Killed {
            by_player: TGameElementID,
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum WorldEventType {
        Entity {
            ent: TGameElementID,
            ty: EntityEvent,
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub struct WorldEvent {
        pub pos: vec2,
        pub ty: WorldEventType,
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum GameEvents {
        World {
            stage_id: TGameElementID,
            events: Vec<WorldEvent>,
        },
    }
}
