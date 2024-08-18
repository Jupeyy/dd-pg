use std::{ops::Deref, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{pool::Pool, recycle::Recycle, traits::Recyclable};

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

pub type PoolArc<T> = Recycle<Arc<PoolArcInner<T>>>;

impl<T> Clone for PoolArc<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            item: self.item.clone(),
        }
    }
}

impl<T> Recyclable for Arc<PoolArcInner<T>> {
    fn new() -> Self {
        Arc::new(PoolArcInner { inner: None })
    }

    fn reset(&mut self) {
        Arc::get_mut(self).unwrap().inner.take();
    }

    fn should_put_to_pool(&self) -> bool {
        Arc::strong_count(self) == 1
    }
}

pub type ArcPool<T> = Pool<Arc<PoolArcInner<T>>>;

impl<T> ArcPool<T> {
    pub fn new_arc(&self, data: T) -> Recycle<Arc<PoolArcInner<T>>> {
        let mut rc = self.new();
        *Arc::get_mut(&mut rc).unwrap() = PoolArcInner { inner: Some(data) };
        rc
    }
}
