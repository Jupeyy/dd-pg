pub mod game_objects {
    use game_interface::types::{emoticons::EnumCount, weapons::WeaponType};
    use hiarc::Hiarc;
    use map::map::groups::layers::tiles::TileBase;
    use math::math::vector::ivec2;
    use shared_base::mapdef_06::EEntityTiles;

    #[derive(Debug, Hiarc, Default)]
    pub struct GameObjectsPickupDefinitions<V> {
        pub hearts: Vec<V>,
        pub shields: Vec<V>,

        pub red_flags: Vec<V>,
        pub blue_flags: Vec<V>,

        pub weapons: [Vec<V>; WeaponType::COUNT],

        pub ninjas: Vec<V>,
    }

    /// definitions of game objects, like their spawn position or flags etc.
    #[derive(Debug, Hiarc)]
    pub struct GameObjectDefinitionsBase<V> {
        pub pickups: GameObjectsPickupDefinitions<V>,
    }

    impl GameObjectDefinitionsBase<ivec2> {
        pub fn new(game_layer_tiles: &[TileBase], width: u32, height: u32) -> Self {
            let mut pickups = GameObjectsPickupDefinitions::<ivec2>::default();

            for y in 0..height {
                for x in 0..width {
                    let tiles = game_layer_tiles;
                    let index = (y * width + x) as usize;
                    match tiles[index].index {
                        i if i == EEntityTiles::Health as u8 => {
                            pickups.hearts.push(ivec2::new(x as i32, y as i32));
                        }
                        i if i == EEntityTiles::Armor as u8 => {
                            pickups.shields.push(ivec2::new(x as i32, y as i32));
                        }
                        i if i == EEntityTiles::FlagSpawnRed as u8 => {
                            pickups.red_flags.push(ivec2::new(x as i32, y as i32));
                        }
                        i if i == EEntityTiles::FlagSpawnBlue as u8 => {
                            pickups.blue_flags.push(ivec2::new(x as i32, y as i32));
                            // TODO remove all as i32, use u16 instead
                        }
                        i if i == EEntityTiles::WeaponGrenade as u8 => {
                            pickups.weapons[WeaponType::Grenade as usize]
                                .push(ivec2::new(x as i32, y as i32));
                        }
                        i if i == EEntityTiles::WeaponLaser as u8 => {
                            pickups.weapons[WeaponType::Laser as usize]
                                .push(ivec2::new(x as i32, y as i32));
                        }
                        i if i == EEntityTiles::WeaponShotgun as u8 => {
                            pickups.weapons[WeaponType::Shotgun as usize]
                                .push(ivec2::new(x as i32, y as i32));
                        }
                        i if i == EEntityTiles::PowerupNinja as u8 => {
                            pickups.ninjas.push(ivec2::new(x as i32, y as i32));
                        }
                        _ => {
                            // not handled
                        }
                    }
                }
            }
            Self { pickups }
        }
    }

    pub type GameObjectDefinitions = GameObjectDefinitionsBase<ivec2>;
}
