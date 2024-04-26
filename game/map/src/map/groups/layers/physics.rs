use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use super::tiles::{
    MapTileLayerPhysicsTilesRef, SpeedupTile, SwitchTile, TeleTile, Tile, TuneTile,
};

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerTilePhysicsBase<T> {
    pub tiles: Vec<T>,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum MapLayerPhysics {
    Arbitrary(Vec<u8>),
    Game(MapLayerTilePhysicsBase<Tile>),
    Front(MapLayerTilePhysicsBase<Tile>),
    Tele(MapLayerTilePhysicsBase<TeleTile>),
    Speedup(MapLayerTilePhysicsBase<SpeedupTile>),
    Switch(MapLayerTilePhysicsBase<SwitchTile>),
    Tune(MapLayerTilePhysicsBase<TuneTile>),
}

#[derive(Debug, Hiarc)]
pub enum MapLayerPhysicsRef<'a> {
    Arbitrary(&'a Vec<u8>),
    Game(&'a MapLayerTilePhysicsBase<Tile>),
    Front(&'a MapLayerTilePhysicsBase<Tile>),
    Tele(&'a MapLayerTilePhysicsBase<TeleTile>),
    Speedup(&'a MapLayerTilePhysicsBase<SpeedupTile>),
    Switch(&'a MapLayerTilePhysicsBase<SwitchTile>),
    Tune(&'a MapLayerTilePhysicsBase<TuneTile>),
}

impl MapLayerPhysics {
    pub fn as_ref(&self) -> MapLayerPhysicsRef {
        match self {
            MapLayerPhysics::Arbitrary(layer) => MapLayerPhysicsRef::Arbitrary(layer),
            MapLayerPhysics::Game(layer) => MapLayerPhysicsRef::Game(layer),
            MapLayerPhysics::Front(layer) => MapLayerPhysicsRef::Front(layer),
            MapLayerPhysics::Tele(layer) => MapLayerPhysicsRef::Tele(layer),
            MapLayerPhysics::Speedup(layer) => MapLayerPhysicsRef::Speedup(layer),
            MapLayerPhysics::Switch(layer) => MapLayerPhysicsRef::Switch(layer),
            MapLayerPhysics::Tune(layer) => MapLayerPhysicsRef::Tune(layer),
        }
    }
}

impl<'a> MapLayerPhysicsRef<'a> {
    pub fn tiles_ref(&self) -> MapTileLayerPhysicsTilesRef {
        match self {
            Self::Arbitrary(_) => panic!("not a tile layer"),
            Self::Game(layer) => MapTileLayerPhysicsTilesRef::Game(&layer.tiles),
            Self::Front(layer) => MapTileLayerPhysicsTilesRef::Front(&layer.tiles),
            Self::Tele(layer) => MapTileLayerPhysicsTilesRef::Tele(&layer.tiles),
            Self::Speedup(layer) => MapTileLayerPhysicsTilesRef::Speedup(&layer.tiles),
            Self::Switch(layer) => MapTileLayerPhysicsTilesRef::Switch(&layer.tiles),
            Self::Tune(layer) => MapTileLayerPhysicsTilesRef::Tune(&layer.tiles),
        }
    }
}
