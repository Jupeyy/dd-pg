use hashlink::LinkedHashMap;
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
pub struct MapLayerTilePhysicsTele {
    pub base: MapLayerTilePhysicsBase<TeleTile>,
    // linked hash map bcs we want to keep order between serialization
    /// Optional names for the teleporters (might include unused ones)
    pub tele_names: LinkedHashMap<u8, String>,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerTilePhysicsSwitch {
    pub base: MapLayerTilePhysicsBase<SwitchTile>,
    // linked hash map bcs we want to keep order between serialization
    /// Optional names for the switches (might include unused ones)
    pub switch_names: LinkedHashMap<u8, String>,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerTilePhysicsTuneZone {
    pub name: String,
    pub tunes: LinkedHashMap<String, String>,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerTilePhysicsTune {
    pub base: MapLayerTilePhysicsBase<TuneTile>,
    // linked hash map bcs we want to keep order between serialization
    pub tune_zones: LinkedHashMap<u8, MapLayerTilePhysicsTuneZone>,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum MapLayerPhysics {
    Arbitrary(Vec<u8>),
    Game(MapLayerTilePhysicsBase<Tile>),
    Front(MapLayerTilePhysicsBase<Tile>),
    Tele(MapLayerTilePhysicsTele),
    Speedup(MapLayerTilePhysicsBase<SpeedupTile>),
    Switch(MapLayerTilePhysicsSwitch),
    Tune(MapLayerTilePhysicsTune),
}

#[derive(Debug, Hiarc)]
pub enum MapLayerPhysicsRef<'a> {
    Arbitrary(&'a Vec<u8>),
    Game(&'a MapLayerTilePhysicsBase<Tile>),
    Front(&'a MapLayerTilePhysicsBase<Tile>),
    Tele(&'a MapLayerTilePhysicsTele),
    Speedup(&'a MapLayerTilePhysicsBase<SpeedupTile>),
    Switch(&'a MapLayerTilePhysicsSwitch),
    Tune(&'a MapLayerTilePhysicsTune),
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
            Self::Tele(layer) => MapTileLayerPhysicsTilesRef::Tele(&layer.base.tiles),
            Self::Speedup(layer) => MapTileLayerPhysicsTilesRef::Speedup(&layer.tiles),
            Self::Switch(layer) => MapTileLayerPhysicsTilesRef::Switch(&layer.base.tiles),
            Self::Tune(layer) => MapTileLayerPhysicsTilesRef::Tune(&layer.base.tiles),
        }
    }
}
