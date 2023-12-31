pub mod game_objects {
    use math::math::vector::ivec2;
    use pool::{datatypes::PoolVec, pool::Pool};
    use shared_base::mapdef::{EEntityTiles, MapLayerTile};

    #[derive(Debug, Default)]
    pub struct GameObjectsPickupDefinitions {
        pub hearts: Vec<ivec2>,
    }

    /// definitions of game objects, like their spawn position or flags etc.
    #[derive(Debug)]
    pub struct GameObjectDefinitions {
        pub pickups: GameObjectsPickupDefinitions,
    }

    impl GameObjectDefinitions {
        pub fn new(layer: &MapLayerTile) -> Self {
            let layer_def = &layer.0;

            let mut pickups = GameObjectsPickupDefinitions::default();

            for y in 0..layer_def.height {
                for x in 0..layer_def.width {
                    let tiles = &layer.2;
                    let index = (y * layer_def.width + x) as usize;
                    if tiles[index].index == EEntityTiles::Health as u8 {
                        pickups.hearts.push(ivec2::new(x, y));
                    }
                }
            }
            Self { pickups }
        }
    }

    #[derive(Debug)]
    pub struct WorldGameObjectsHeart {
        pub pos: ivec2,
    }

    #[derive(Debug)]
    pub struct WorldGameObjectsPickups {
        pub hearts: PoolVec<WorldGameObjectsHeart>,
    }

    #[derive(Debug)]
    pub struct WorldGameObjects {
        pub pickups: WorldGameObjectsPickups,
    }

    #[derive(Debug, Clone)]
    pub struct WorldGameObjectsPickupsPool {
        pub hearts: Pool<Vec<WorldGameObjectsHeart>>,
    }

    #[derive(Debug, Clone)]
    pub struct WorldGameObjectsPool {
        pub pickups: WorldGameObjectsPickupsPool,
    }
}
