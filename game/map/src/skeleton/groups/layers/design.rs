use hiarc::Hiarc;

use crate::map::groups::layers::design::{MapLayer, MapLayerQuad, MapLayerSound, MapLayerTile};

#[derive(Debug, Hiarc, Clone)]
pub struct MapLayerArbitrarySkeleton<A> {
    pub buf: Vec<u8>,
    pub user: A,
}

impl<A> From<MapLayerArbitrarySkeleton<A>> for Vec<u8> {
    fn from(value: MapLayerArbitrarySkeleton<A>) -> Self {
        value.buf
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapLayerTileSkeleton<T> {
    pub layer: MapLayerTile,
    pub user: T,
}

impl<A> From<MapLayerTileSkeleton<A>> for MapLayerTile {
    fn from(value: MapLayerTileSkeleton<A>) -> Self {
        value.layer
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapLayerQuadSkeleton<Q> {
    pub layer: MapLayerQuad,
    pub user: Q,
}

impl<A> From<MapLayerQuadSkeleton<A>> for MapLayerQuad {
    fn from(value: MapLayerQuadSkeleton<A>) -> Self {
        value.layer
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapLayerSoundSkeleton<S> {
    pub layer: MapLayerSound,
    pub user: S,
}

impl<A> From<MapLayerSoundSkeleton<A>> for MapLayerSound {
    fn from(value: MapLayerSoundSkeleton<A>) -> Self {
        value.layer
    }
}

#[derive(Debug, Hiarc, Clone)]
pub enum MapLayerSkeleton<T, Q, S, A> {
    /// can be used for mods, if client compability is important, while having custom layers
    Abritrary(MapLayerArbitrarySkeleton<A>),
    Tile(MapLayerTileSkeleton<T>),
    Quad(MapLayerQuadSkeleton<Q>),
    Sound(MapLayerSoundSkeleton<S>),
}

impl<T, Q, S, A> From<MapLayerSkeleton<T, Q, S, A>> for MapLayer {
    fn from(value: MapLayerSkeleton<T, Q, S, A>) -> Self {
        match value {
            MapLayerSkeleton::Abritrary(layer) => MapLayer::Abritrary(layer.into()),
            MapLayerSkeleton::Tile(layer) => MapLayer::Tile(layer.into()),
            MapLayerSkeleton::Quad(layer) => MapLayer::Quad(layer.into()),
            MapLayerSkeleton::Sound(layer) => MapLayer::Sound(layer.into()),
        }
    }
}

impl<T, Q, S, A> MapLayerSkeleton<T, Q, S, A> {
    pub fn high_detail(&self) -> bool {
        match self {
            MapLayerSkeleton::Abritrary(_) => false,
            MapLayerSkeleton::Tile(MapLayerTileSkeleton { layer, .. }) => layer.attr.high_detail,
            MapLayerSkeleton::Quad(MapLayerQuadSkeleton { layer, .. }) => layer.attr.high_detail,
            MapLayerSkeleton::Sound(MapLayerSoundSkeleton { layer, .. }) => layer.attr.high_detail,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            MapLayerSkeleton::Abritrary(_) => "unknown layer",
            MapLayerSkeleton::Tile(layer) => &layer.layer.name,
            MapLayerSkeleton::Quad(layer) => &layer.layer.name,
            MapLayerSkeleton::Sound(layer) => &layer.layer.name,
        }
    }
}
