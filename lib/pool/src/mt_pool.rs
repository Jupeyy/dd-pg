use std::sync::Arc;

use crate::{mt_recycle::Recycle, traits::Recyclable};

/// Thread-safe wait-free version of the pool
#[derive(Debug)]
pub struct Pool<T: Recyclable + Send> {
    pub(crate) pool: Arc<spin::Mutex<Vec<T>>>,
}

impl<T: Recyclable + Send> Pool<T> {
    /// If capacity is 0, the pool will not allocate memory for any elements, but will still create heap memory.
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

impl<T: Recyclable + Send> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
