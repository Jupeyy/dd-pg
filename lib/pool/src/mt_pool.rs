use std::{
    mem::ManuallyDrop,
    sync::{atomic::AtomicUsize, Arc},
};

use crate::{mt_recycle::Recycle, traits::Recyclable};

#[cfg_attr(feature = "enable_hiarc", derive(hiarc::Hiarc))]
#[derive(Debug)]
pub(crate) struct PoolInner<T: Recyclable + Send> {
    pool: parking_lot::Mutex<Vec<T>>,
    lock_free_counter: AtomicUsize,
}

impl<T: Recyclable + Send> PoolInner<T> {
    pub(crate) fn take(&self) -> Vec<T> {
        let mut pool = self.pool.lock();
        let res = std::mem::take(&mut *pool);
        self.lock_free_counter
            .store(0, std::sync::atomic::Ordering::SeqCst);
        res
    }

    pub(crate) fn push(&self, item: T) {
        let mut pool = self.pool.lock();
        self.lock_free_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        pool.push(item);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lock_free_counter
            .load(std::sync::atomic::Ordering::SeqCst)
            == 0
    }
}

/// Thread-safe version of the pool.
#[cfg_attr(feature = "enable_hiarc", derive(hiarc::Hiarc))]
#[derive(Debug)]
pub struct Pool<T: Recyclable + Send> {
    pub(crate) pool: Arc<PoolInner<T>>,
}

impl<T: Recyclable + Send> Pool<T> {
    /// If capacity is 0, the pool will not allocate memory for any elements, but will still create heap memory.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pool: Arc::new(PoolInner {
                pool: parking_lot::Mutex::new(Vec::with_capacity(capacity)),
                lock_free_counter: AtomicUsize::new(0),
            }),
        }
    }

    pub fn with_sized<F>(new_size: usize, item_constructor: F) -> Self
    where
        F: FnMut() -> T,
    {
        let res = Self {
            pool: Arc::new(PoolInner {
                pool: parking_lot::Mutex::new(Vec::with_capacity(new_size)),
                lock_free_counter: AtomicUsize::new(new_size),
            }),
        };
        res.pool.pool.lock().resize_with(new_size, item_constructor);
        res
    }

    pub fn new(&self) -> Recycle<T> {
        if let Some(item) = self.pool.pool.lock().pop() {
            self.pool
                .lock_free_counter
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Recycle {
                pool: Some(self.pool.clone()),
                item: ManuallyDrop::new(item),
            }
        } else {
            Recycle {
                pool: Some(self.pool.clone()),
                item: ManuallyDrop::new(T::new()),
            }
        }
    }

    pub fn items_in_pool(&self) -> usize {
        self.pool
            .lock_free_counter
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl<T: Recyclable + Send> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
