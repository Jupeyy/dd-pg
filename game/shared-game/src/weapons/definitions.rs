pub mod weapon_def {
    use game_interface::types::{
        emoticons::EnumCount, game::GameTickCooldown, weapons::WeaponType,
    };
    use hiarc::Hiarc;
    use math::math::{length, vector::vec2};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Hiarc, Copy, Clone, Default, Serialize, Deserialize)]
    pub struct Weapon {
        pub next_ammo_regeneration_tick: GameTickCooldown,
        /// A value of `None` here means unlimited ammo
        pub cur_ammo: Option<u32>,
    }

    pub const WEAPON_VISUAL_SIZES: [f32; WeaponType::COUNT] = [3.0, 2.0, 3.0, 3.0, 92.0 / 32.0];
    pub const NINJA_PICKUP_VISUAL_SIZE: f32 = 4.0;
    pub const NINJA_WEAPON_VISUAL_SIZE: f32 = 3.0;

    pub const WEAPON_SCALES: [(usize, usize); WeaponType::COUNT] =
        [(4, 3), (4, 2), (8, 2), (7, 2), (7, 3)];
    pub const NINJA_PICKUP_VISUAL_SCALE: (usize, usize) = (8, 2);

    pub fn get_weapon_visual_scale(weapon: &WeaponType) -> f32 {
        WEAPON_VISUAL_SIZES[*weapon as usize]
    }

    pub fn get_scale(x: f32, y: f32) -> (f32, f32) {
        let f = length(&vec2::new(x, y));
        (x / f, y / f)
    }

    pub fn get_weapon_sprite_scale(weapon: &WeaponType) -> (f32, f32) {
        let scale = WEAPON_SCALES[*weapon as usize];
        get_scale(scale.0 as f32, scale.1 as f32)
    }

    pub fn get_ninja_sprite_scale() -> (f32, f32) {
        let scale = NINJA_PICKUP_VISUAL_SCALE;
        let f = length(&vec2::new(scale.0 as f32, scale.1 as f32));
        (scale.0 as f32 / f, scale.1 as f32 / f)
    }
}
