use hiarc::Hiarc;

use crate::map::metadata::Metadata;

#[derive(Debug, Hiarc, Clone)]
pub struct MetadataSkeleton<M> {
    pub def: Metadata,
    pub user: M,
}

impl<M> From<MetadataSkeleton<M>> for Metadata {
    fn from(value: MetadataSkeleton<M>) -> Self {
        value.def
    }
}
