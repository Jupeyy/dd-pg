use hiarc::Hiarc;

use crate::map::groups::{MapGroup, MapGroupAttr, MapGroupPhysics, MapGroupPhysicsAttr, MapGroups};

use self::layers::{design::MapLayerSkeleton, physics::MapLayerPhysicsSkeleton};

pub mod layers;

#[derive(Debug, Hiarc, Clone)]
pub struct MapGroupPhysicsSkeleton<G, L> {
    pub attr: MapGroupPhysicsAttr,
    pub layers: Vec<MapLayerPhysicsSkeleton<L>>,
    pub user: G,
}

impl<G, L> From<MapGroupPhysicsSkeleton<G, L>> for MapGroupPhysics {
    fn from(value: MapGroupPhysicsSkeleton<G, L>) -> Self {
        Self {
            attr: value.attr,
            layers: value.layers.into_iter().map(|i| i.into()).collect(),
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapGroupSkeleton<G, T, Q, S, A> {
    pub attr: MapGroupAttr,
    pub layers: Vec<MapLayerSkeleton<T, Q, S, A>>,

    /// optional name, mostly intersting for editor
    pub name: String,

    pub user: G,
}

impl<G, T, Q, S, A> From<MapGroupSkeleton<G, T, Q, S, A>> for MapGroup {
    fn from(value: MapGroupSkeleton<G, T, Q, S, A>) -> Self {
        Self {
            attr: value.attr,
            layers: value.layers.into_iter().map(|i| i.into()).collect(),
            name: value.name,
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapGroupsSkeleton<GS, PG, PL, G, T, Q, S, A> {
    pub physics: MapGroupPhysicsSkeleton<PG, PL>,

    pub background: Vec<MapGroupSkeleton<G, T, Q, S, A>>,
    pub foreground: Vec<MapGroupSkeleton<G, T, Q, S, A>>,

    pub user: GS,
}

impl<GS, PG, PL, G, T, Q, S, A> From<MapGroupsSkeleton<GS, PG, PL, G, T, Q, S, A>> for MapGroups {
    fn from(value: MapGroupsSkeleton<GS, PG, PL, G, T, Q, S, A>) -> Self {
        Self {
            physics: value.physics.into(),
            background: value.background.into_iter().map(|i| i.into()).collect(),
            foreground: value.foreground.into_iter().map(|i| i.into()).collect(),
        }
    }
}
