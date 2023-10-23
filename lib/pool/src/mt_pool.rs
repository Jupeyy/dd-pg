use std::sync::Arc;

use crate::{mt_recycle::Recycle, traits::Recyclable};

/// Thread-safe lock-free version of the pool
#[derive(Debug, Clone)]
pub struct Pool<T: Recyclable + Send> {
    pub(crate) pool: Arc<spin::Mutex<Vec<T>>>,
}

impl<T: Recyclable + Send> Pool<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pool: Arc::new(spin::Mutex::new(Vec::with_capacity(capacity))),
        }
    }

    pub fn with_sized<F>(new_size: usize, item_constructor: F) -> Self
    where
        F: FnMut() -> T,
    {
        let res = Self {
            pool: Arc::new(spin::Mutex::new(Vec::with_capacity(new_size))),
        };
        res.pool.lock().resize_with(new_size, item_constructor);
        res
    }

    pub fn new(&self) -> Recycle<T> {
        if let Some(item) = self.pool.lock().pop() {
            Recycle {
                pool: Some(self.pool.clone()),
                item,
            }
        } else {
            Recycle {
                pool: Some(self.pool.clone()),
                item: T::new(),
            }
        }
    }

    pub fn items_in_pool(&self) -> usize {
        self.pool.lock().len()
    }
}
