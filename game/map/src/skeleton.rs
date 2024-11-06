pub mod animations;
pub mod config;
pub mod groups;
pub mod metadata;
pub mod resources;

use hiarc::Hiarc;

use crate::map::Map;

use self::{
    animations::AnimationsSkeleton, config::ConfigSkeleton, groups::MapGroupsSkeleton,
    metadata::MetadataSkeleton, resources::MapResourcesSkeleton,
};

/// basically a wrapper around the all map data types with custom properties
/// additionally it does not require the generics to have serialization support
#[derive(Debug, Hiarc, Clone)]
pub struct MapSkeleton<E, R, RI, RI2, RS, GS, PG, PL, G, T, Q, S, CA, AS, A, C, M> {
    pub resources: MapResourcesSkeleton<R, RI, RI2, RS>,
    pub groups: MapGroupsSkeleton<GS, PG, PL, G, T, Q, S, CA>,
    pub animations: AnimationsSkeleton<AS, A>,
    pub config: ConfigSkeleton<C>,
    pub meta: MetadataSkeleton<M>,

    pub user: E,
}

impl<E, R, RI, RI2, RS, GS, PG, PL, G, T, Q, S, CA, AS, A, C, M>
    From<MapSkeleton<E, R, RI, RI2, RS, GS, PG, PL, G, T, Q, S, CA, AS, A, C, M>> for Map
{
    fn from(
        value: MapSkeleton<E, R, RI, RI2, RS, GS, PG, PL, G, T, Q, S, CA, AS, A, C, M>,
    ) -> Self {
        Map {
            resources: value.resources.into(),
            groups: value.groups.into(),
            animations: value.animations.into(),
            config: value.config.into(),
            meta: value.meta.into(),
        }
    }
}
