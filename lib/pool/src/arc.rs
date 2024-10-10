use std::{mem::ManuallyDrop, ops::Deref, sync::Arc};

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::{mt_pool::Pool, mt_recycle::Recycle, traits::Recyclable};

#[cfg_attr(feature = "enable_hiarc", derive(hiarc::Hiarc))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PoolArcInner<T> {
    inner: Option<T>,
}

impl<T> Deref for PoolArcInner<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct ArcManualClone<T>(pub(crate) Arc<T>);

impl<T> Deref for ArcManualClone<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type PoolArc<T> = Recycle<ArcManualClone<PoolArcInner<T>>>;

impl<T: Send + Sync> PoolArc<T> {
    pub fn from_item_without_pool(data: T) -> Self {
        Self {
            pool: None,
            item: ManuallyDrop::new(ArcManualClone(Arc::new(PoolArcInner { inner: Some(data) }))),
        }
    }
}

impl<T> Recyclable for ArcManualClone<PoolArcInner<T>> {
    fn new() -> Self {
        ArcManualClone(Arc::new(PoolArcInner { inner: None }))
    }

    fn reset(&mut self) {
        Arc::get_mut(&mut self.0).unwrap().inner.take();
    }

    fn should_put_to_pool(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }
}

pub type ArcPool<T> = Pool<ArcManualClone<PoolArcInner<T>>>;

impl<T: Send + Sync> ArcPool<T> {
    pub fn new_arc(&self, data: T) -> Recycle<ArcManualClone<PoolArcInner<T>>> {
        let mut arc = self.new();
        *Arc::get_mut(&mut arc.0).unwrap() = PoolArcInner { inner: Some(data) };
        arc
    }
}
