use std::{mem::ManuallyDrop, ops::Deref, rc::Rc};

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use crate::{pool::Pool, recycle::Recycle, traits::Recyclable};

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "enable_hiarc", derive(hiarc::Hiarc))]
pub struct PoolRcInner<T> {
    inner: Option<T>,
}

impl<T> Deref for PoolRcInner<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct RcManualClone<T>(pub(crate) Rc<T>);

impl<T> Deref for RcManualClone<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type PoolRc<T> = Recycle<RcManualClone<PoolRcInner<T>>>;

impl<T> PoolRc<T> {
    pub fn from_item_without_pool(data: T) -> Self {
        Self {
            pool: None,
            item: ManuallyDrop::new(RcManualClone(Rc::new(PoolRcInner { inner: Some(data) }))),
        }
    }
}

impl<T> Recyclable for RcManualClone<PoolRcInner<T>> {
    fn new() -> Self {
        RcManualClone(Rc::new(PoolRcInner { inner: None }))
    }

    fn reset(&mut self) {
        Rc::get_mut(&mut self.0).unwrap().inner.take();
    }

    fn should_put_to_pool(&self) -> bool {
        Rc::strong_count(&self.0) == 1
    }
}

pub type RcPool<T> = Pool<RcManualClone<PoolRcInner<T>>>;

impl<T> RcPool<T> {
    pub fn new_rc(&self, data: T) -> Recycle<RcManualClone<PoolRcInner<T>>> {
        let mut rc = self.new();
        *Rc::get_mut(&mut rc.0).unwrap() = PoolRcInner { inner: Some(data) };
        rc
    }
}
