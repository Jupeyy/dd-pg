pub mod weapon_def {
    use bincode::{Decode, Encode};
    use math::math::{length, vector::vec2};
    use num_derive::FromPrimitive;
    use serde::{Deserialize, Serialize};

    use shared_base::types::GameTickType;

    #[derive(
        Debug,
        Default,
        Copy,
        Clone,
        PartialEq,
        Eq,
        FromPrimitive,
        Serialize,
        Deserialize,
        Encode,
        Decode,
        Hash,
        PartialOrd,
    )]
    pub enum WeaponType {
        Hammer = 0,
        Gun,
        Shotgun,
        Grenade,
        Laser,
        Ninja,

        NumWeapons,
        MaxWeapons = 10,
        #[default]
        Invalid = 0xFFFF,
    }

    impl WeaponType {
        pub fn to_string(&self) -> &'static str {
            match self {
                WeaponType::Hammer => "hammer",
                WeaponType::Gun => "gun",
                WeaponType::Shotgun => "shotgun",
                WeaponType::Grenade => "grenade",
                WeaponType::Laser => "laser",
                WeaponType::Ninja => todo!(),
                WeaponType::NumWeapons => todo!(),
                WeaponType::MaxWeapons => todo!(),
                WeaponType::Invalid => todo!(),
            }
        }
    }

    #[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, Encode, Decode)]
    pub struct Weapon {
        pub next_ammo_regeneration_tick: GameTickType,
        pub cur_ammo: u32,
    }

    pub const WEAPON_VISUAL_SIZES: [f32; WeaponType::NumWeapons as usize] =
        [96.0, 64.0, 96.0, 96.0, 92.0, 96.0];

    pub const WEAPON_SCALES: [(usize, usize); WeaponType::NumWeapons as usize] =
        [(4, 3), (4, 2), (8, 2), (7, 2), (7, 3), (8, 2)];

    pub fn get_weapon_visual_scale(weapon: &WeaponType) -> f32 {
        WEAPON_VISUAL_SIZES[*weapon as usize]
    }

    pub fn get_weapon_sprite_scale(weapon: &WeaponType) -> (f32, f32) {
        let scale = WEAPON_SCALES[*weapon as usize];
        let f = length(&vec2::new(scale.0 as f32, scale.1 as f32));
        (scale.0 as f32 / f, scale.1 as f32 / f)
    }
}
