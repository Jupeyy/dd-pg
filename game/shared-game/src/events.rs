pub mod events {
    use bincode::{Decode, Encode};
    use math::math::vector::vec2;
    use shared_base::{game_types::TGameElementID, types::GameTickType};

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum EntitiyEvent {
        Die {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
        },
        Projectile {
            pos: vec2,
            dir: vec2,
        },
        Laser {
            pos: vec2,
            dir: vec2,
        },
        Sound {
            // TODO:
        },
        Explosion {
            // TODO:
        },
    }

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum WorldEventType {
        Entity {
            ent: TGameElementID,
            ty: EntitiyEvent,
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
