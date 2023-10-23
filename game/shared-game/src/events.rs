pub mod events {
    use bincode::{Decode, Encode};
    use math::math::vector::vec2;
    use shared_base::{
        game_types::TGameElementID, network::messages::WeaponType, types::GameTickType,
    };

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum EntityEvent {
        Die {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
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
        Sound {
            pos: vec2,
            name: String, // TODO: stack string? pool string?
        },
        Explosion {
            // TODO:
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
