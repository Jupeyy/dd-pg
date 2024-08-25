use std::{mem::ManuallyDrop, ops::Deref, rc::Rc};

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

pub type PoolRc<T> = Recycle<Rc<PoolRcInner<T>>>;

impl<T> Clone for PoolRc<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            item: self.item.clone(),
        }
    }
}

impl<T> PoolRc<T> {
    pub fn from_item_without_pool(data: T) -> Self {
        Self {
            pool: None,
            item: ManuallyDrop::new(Rc::new(PoolRcInner { inner: Some(data) })),
        }
    }
}

impl<T> Recyclable for Rc<PoolRcInner<T>> {
    fn new() -> Self {
        Rc::new(PoolRcInner { inner: None })
    }

    fn reset(&mut self) {
        Rc::get_mut(self).unwrap().inner.take();
    }

    fn should_put_to_pool(&self) -> bool {
        Rc::strong_count(self) == 1
    }
}

pub type RcPool<T> = Pool<Rc<PoolRcInner<T>>>;

impl<T> RcPool<T> {
    pub fn new_rc(&self, data: T) -> Recycle<Rc<PoolRcInner<T>>> {
        let mut rc = self.new();
        *Rc::get_mut(&mut rc).unwrap() = PoolRcInner { inner: Some(data) };
        rc
    }
}
