pub mod events {
    use game_interface::{
        events::{
            GameBuffEffectEvent, GameBuffSoundEvent, GameCharacterEventEffect,
            GameCharacterEventSound, GameDebuffEffectEvent, GameDebuffSoundEvent,
            GameFlagEventEffect, GameFlagEventSound, GameGrenadeEventEffect, GameGrenadeEventSound,
            GameLaserEventSound, GameWorldActionKillWeapon,
        },
        types::{
            flag::FlagType, game::GameTickCooldown, id_types::CharacterId, pickup::PickupType,
            weapons::WeaponType,
        },
    };
    use hiarc::Hiarc;
    use math::math::vector::vec2;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
    pub enum ProjectileEvent {
        Despawn {
            pos: vec2,
            respawns_in_ticks: GameTickCooldown,
        },
        GrenadeSound {
            pos: Option<vec2>,
            ev: GameGrenadeEventSound,
        },
        GrenadeEffect {
            pos: vec2,
            ev: GameGrenadeEventEffect,
        },
    }

    #[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
    pub enum LaserEvent {
        Despawn {
            pos: vec2,
            respawns_in_ticks: GameTickCooldown,
        },
        Sound {
            pos: Option<vec2>,
            ev: GameLaserEventSound,
        },
    }

    #[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
    pub enum PickupEvent {
        Despawn {
            pos: vec2,
            ty: PickupType,
            respawns_in_ticks: GameTickCooldown,
        },
        Pickup {
            pos: vec2,
            ty: PickupType,
        },
    }

    #[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
    pub enum FlagEvent {
        Despawn {
            pos: vec2,
            ty: FlagType,
            respawns_in_ticks: GameTickCooldown,
        },
        Sound {
            pos: Option<vec2>,
            ev: GameFlagEventSound,
        },
        Effect {
            pos: vec2,
            ev: GameFlagEventEffect,
        },
        Capture {
            pos: vec2,
        },
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub struct CharacterDespawnInfo {
        pub pos: vec2,
        pub respawns_in_ticks: GameTickCooldown,
        pub killer_id: Option<CharacterId>,
        pub weapon: GameWorldActionKillWeapon,
    }

    #[derive(Debug, Hiarc, Default)]
    pub enum CharacterDespawnType {
        #[default]
        DropFromGame,
        Default(CharacterDespawnInfo),
        JoinsSpectator,
    }

    #[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
    pub enum CharacterEvent {
        Despawn {
            killer_id: Option<CharacterId>,
            weapon: GameWorldActionKillWeapon,
        },
        Sound {
            pos: Option<vec2>,
            ev: GameCharacterEventSound,
        },
        /// A visual effect
        Effect {
            pos: vec2,
            ev: GameCharacterEventEffect,
        },
        BuffSound {
            pos: Option<vec2>,
            ev: GameBuffSoundEvent,
        },
        BuffEffect {
            pos: vec2,
            ev: GameBuffEffectEvent,
        },
        DebuffSound {
            pos: Option<vec2>,
            ev: GameDebuffSoundEvent,
        },
        DebuffEffect {
            pos: vec2,
            ev: GameDebuffEffectEvent,
        },
    }

    #[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
    pub enum CharacterTickEvent {
        Projectile {
            pos: vec2,
            dir: vec2,
            ty: WeaponType,
            lifetime: f32,
        },
        Laser {
            pos: vec2,
            dir: vec2,
            energy: f32,
            can_hit_own: bool,
        },
    }
}
