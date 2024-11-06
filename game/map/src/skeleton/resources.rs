use hiarc::Hiarc;

use crate::map::resources::{MapResourceRef, Resources};

#[derive(Debug, Hiarc, Clone)]
pub struct MapResourceRefSkeleton<R> {
    pub def: MapResourceRef,
    pub user: R,
}

impl<R> From<MapResourceRefSkeleton<R>> for MapResourceRef {
    fn from(value: MapResourceRefSkeleton<R>) -> Self {
        value.def
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct MapResourcesSkeleton<R, RI, RI2, RS> {
    pub images: Vec<MapResourceRefSkeleton<RI>>,
    pub image_arrays: Vec<MapResourceRefSkeleton<RI2>>,
    pub sounds: Vec<MapResourceRefSkeleton<RS>>,

    pub user: R,
}

impl<R, RI, RI2, RS> From<MapResourcesSkeleton<R, RI, RI2, RS>> for Resources {
    fn from(value: MapResourcesSkeleton<R, RI, RI2, RS>) -> Self {
        Self {
            images: value.images.into_iter().map(|i| i.into()).collect(),
            image_arrays: value.image_arrays.into_iter().map(|i| i.into()).collect(),
            sounds: value.sounds.into_iter().map(|i| i.into()).collect(),
        }
    }
}
