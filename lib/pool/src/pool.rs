use std::{cell::RefCell, mem::ManuallyDrop, rc::Rc};

use crate::{recycle::Recycle, traits::Recyclable};

// No crate fulfilled our requirements => so own implementation.
/// We want a pool with elements where T is trivially creatable,
/// so that we can store the whole object and pool as object
/// with automatic cleanup, no lifetimes etc.
///
/// Additionally it supports having no pool to recycle to.
#[cfg_attr(feature = "enable_hiarc", derive(hiarc::Hiarc))]
#[derive(Debug)]
pub struct Pool<T: Recyclable> {
    pub(crate) pool: Rc<RefCell<Vec<T>>>,
}

impl<T: Recyclable> Pool<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pool: Rc::new(RefCell::new(Vec::with_capacity(capacity))),
        }
    }

    pub fn with_sized<F>(new_size: usize, item_constructor: F) -> Self
    where
        F: FnMut() -> T,
    {
        let res = Self {
            pool: Rc::new(RefCell::new(Vec::with_capacity(new_size))),
        };
        res.pool
            .borrow_mut()
            .resize_with(new_size, item_constructor);
        res
    }

    pub fn new(&self) -> Recycle<T> {
        let mut pool = self.pool.borrow_mut();
        if let Some(item) = pool.pop() {
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
        self.pool.borrow().len()
    }
}

impl<T: Recyclable> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
