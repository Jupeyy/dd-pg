use hiarc::Hiarc;

use crate::map::groups::layers::{
    physics::{MapLayerPhysics, MapLayerPhysicsRef, MapLayerTilePhysicsBase},
    tiles::{SpeedupTile, SwitchTile, TeleTile, Tile, TuneTile},
};

#[derive(Debug, Hiarc, Clone)]
pub struct MapLayerTilePhysicsBaseSkeleton<T, L> {
    pub def: MapLayerTilePhysicsBase<T>,
    pub user: L,
}

impl<T, L> From<MapLayerTilePhysicsBaseSkeleton<T, L>> for MapLayerTilePhysicsBase<T> {
    fn from(value: MapLayerTilePhysicsBaseSkeleton<T, L>) -> Self {
        value.def
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapLayerArbitraryPhysicsSkeleton<L> {
    pub buf: Vec<u8>,
    pub user: L,
}

impl<L> From<MapLayerArbitraryPhysicsSkeleton<L>> for Vec<u8> {
    fn from(value: MapLayerArbitraryPhysicsSkeleton<L>) -> Self {
        value.buf
    }
}

#[derive(Debug, Hiarc, Clone)]
pub enum MapLayerPhysicsSkeleton<L> {
    Arbitrary(MapLayerArbitraryPhysicsSkeleton<L>),
    Game(MapLayerTilePhysicsBaseSkeleton<Tile, L>),
    Front(MapLayerTilePhysicsBaseSkeleton<Tile, L>),
    Tele(MapLayerTilePhysicsBaseSkeleton<TeleTile, L>),
    Speedup(MapLayerTilePhysicsBaseSkeleton<SpeedupTile, L>),
    Switch(MapLayerTilePhysicsBaseSkeleton<SwitchTile, L>),
    Tune(MapLayerTilePhysicsBaseSkeleton<TuneTile, L>),
}

impl<L> From<MapLayerPhysicsSkeleton<L>> for MapLayerPhysics {
    fn from(value: MapLayerPhysicsSkeleton<L>) -> Self {
        match value {
            MapLayerPhysicsSkeleton::Arbitrary(layer) => MapLayerPhysics::Arbitrary(layer.into()),
            MapLayerPhysicsSkeleton::Game(layer) => MapLayerPhysics::Game(layer.into()),
            MapLayerPhysicsSkeleton::Front(layer) => MapLayerPhysics::Front(layer.into()),
            MapLayerPhysicsSkeleton::Tele(layer) => MapLayerPhysics::Tele(layer.into()),
            MapLayerPhysicsSkeleton::Speedup(layer) => MapLayerPhysics::Speedup(layer.into()),
            MapLayerPhysicsSkeleton::Switch(layer) => MapLayerPhysics::Switch(layer.into()),
            MapLayerPhysicsSkeleton::Tune(layer) => MapLayerPhysics::Tune(layer.into()),
        }
    }
}

impl<L> MapLayerPhysicsSkeleton<L> {
    pub fn user(&self) -> &L {
        match self {
            MapLayerPhysicsSkeleton::Arbitrary(layer) => &layer.user,
            MapLayerPhysicsSkeleton::Game(layer) => &layer.user,
            MapLayerPhysicsSkeleton::Front(layer) => &layer.user,
            MapLayerPhysicsSkeleton::Tele(layer) => &layer.user,
            MapLayerPhysicsSkeleton::Speedup(layer) => &layer.user,
            MapLayerPhysicsSkeleton::Switch(layer) => &layer.user,
            MapLayerPhysicsSkeleton::Tune(layer) => &layer.user,
        }
    }
    pub fn user_mut(&mut self) -> &mut L {
        match self {
            MapLayerPhysicsSkeleton::Arbitrary(layer) => &mut layer.user,
            MapLayerPhysicsSkeleton::Game(layer) => &mut layer.user,
            MapLayerPhysicsSkeleton::Front(layer) => &mut layer.user,
            MapLayerPhysicsSkeleton::Tele(layer) => &mut layer.user,
            MapLayerPhysicsSkeleton::Speedup(layer) => &mut layer.user,
            MapLayerPhysicsSkeleton::Switch(layer) => &mut layer.user,
            MapLayerPhysicsSkeleton::Tune(layer) => &mut layer.user,
        }
    }
    pub fn layer_ref(&self) -> MapLayerPhysicsRef {
        match self {
            MapLayerPhysicsSkeleton::Arbitrary(layer) => MapLayerPhysicsRef::Arbitrary(&layer.buf),
            MapLayerPhysicsSkeleton::Game(layer) => MapLayerPhysicsRef::Game(&layer.def),
            MapLayerPhysicsSkeleton::Front(layer) => MapLayerPhysicsRef::Front(&layer.def),
            MapLayerPhysicsSkeleton::Tele(layer) => MapLayerPhysicsRef::Tele(&layer.def),
            MapLayerPhysicsSkeleton::Speedup(layer) => MapLayerPhysicsRef::Speedup(&layer.def),
            MapLayerPhysicsSkeleton::Switch(layer) => MapLayerPhysicsRef::Switch(&layer.def),
            MapLayerPhysicsSkeleton::Tune(layer) => MapLayerPhysicsRef::Tune(&layer.def),
        }
    }
}
