use std::mem::ManuallyDrop;

use crate::traits::Recyclable;

impl<T: Recyclable + Send + Clone> Clone for crate::mt_recycle::Recycle<T> {
    fn clone(&self) -> Self {
        let pool = self.pool.clone();
        let mut item = pool.as_ref().map(|p| p.get()).unwrap_or_else(|| T::new());
        item.reset();
        item.clone_from(&self.item);
        crate::mt_recycle::Recycle {
            pool,
            item: ManuallyDrop::new(item),
        }
    }

    fn clone_from(&mut self, other: &Self) {
        self.item.clone_from(&other.item)
    }
}

impl<T: Recyclable + Clone> Clone for crate::recycle::Recycle<T> {
    fn clone(&self) -> Self {
        let pool = self.pool.clone();
        let mut item = pool
            .as_ref()
            .and_then(|p| p.borrow_mut().pop())
            .unwrap_or_else(|| T::new());
        item.reset();
        item.clone_from(&self.item);
        crate::recycle::Recycle {
            pool,
            item: ManuallyDrop::new(item),
        }
    }

    fn clone_from(&mut self, other: &Self) {
        self.item.clone_from(&other.item)
    }
}

impl<T> Clone for crate::recycle::Recycle<crate::rc::RcManualClone<crate::rc::PoolRcInner<T>>> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            item: ManuallyDrop::new(crate::rc::RcManualClone(self.item.0.clone())),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }
}

impl<T: Send + Sync> Clone
    for crate::mt_recycle::Recycle<crate::arc::ArcManualClone<crate::arc::PoolArcInner<T>>>
{
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            item: ManuallyDrop::new(crate::arc::ArcManualClone(self.item.0.clone())),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }
}
