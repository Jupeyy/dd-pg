use std::{ops::Deref, rc::Rc};

use crate::{pool::Pool, recycle::Recycle, traits::Recyclable};

#[derive(Debug)]
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

#[cfg(feature = "enable_hiarc")]
impl<T: hiarc::HiarcTrait> hiarc::HiarcTrait for PoolRcInner<T> {
    const HI_VAL: u64 = T::HI_VAL;
}

#[cfg(feature = "enable_hiarc")]
pub type PoolHiRc<T> = Recycle<hiarc::HiRc<PoolRcInner<T>>>;

#[cfg(feature = "enable_hiarc")]
impl<T: hiarc::HiarcTrait> Clone for PoolHiRc<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            item: self.item.clone(),
        }
    }
}

#[cfg(feature = "enable_hiarc")]
impl<T: hiarc::HiarcTrait> Recyclable for hiarc::HiRc<PoolRcInner<T>> {
    fn new() -> Self {
        hiarc::HiRc::new(PoolRcInner { inner: None })
    }

    fn reset(&mut self) {
        Rc::get_mut(self.inner_rc_mut()).unwrap().inner.take();
    }

    fn should_put_to_pool(&self) -> bool {
        Rc::strong_count(&self.inner_rc()) == 1
    }
}

#[cfg(feature = "enable_hiarc")]
pub type HiRcPool<T> = Pool<hiarc::HiRc<PoolRcInner<T>>>;

#[cfg(feature = "enable_hiarc")]
impl<T: hiarc::HiarcTrait> HiRcPool<T> {
    pub fn new_rc(&self, data: T) -> Recycle<hiarc::HiRc<PoolRcInner<T>>> {
        let mut rc = self.new();
        *Rc::get_mut(rc.inner_rc_mut()).unwrap() = PoolRcInner { inner: Some(data) };
        rc
    }
}
