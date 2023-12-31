use std::{fmt::Debug, rc::Rc};

pub trait SharedIndexCleanup: Debug {
    fn destroy_from_index(&self, index: u128);
}

/**
 * A helper struct for resources
 * that are shared.
 * E.g. textures, buffers etc.
 */
#[derive(Debug)]
pub struct SharedIndex {
    index: Rc<u128>,
    dealloc: Rc<dyn SharedIndexCleanup>,
}

impl Clone for SharedIndex {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            dealloc: self.dealloc.clone(),
        }
    }
}

impl SharedIndex {
    pub fn new<F>(index: u128, dealloc: Rc<F>) -> Self
    where
        F: SharedIndexCleanup + 'static,
    {
        Self {
            index: Rc::new(index),
            dealloc,
        }
    }
}

impl PartialEq for SharedIndex {
    fn eq(&self, other: &Self) -> bool {
        self.index.eq(&other.index)
    }
}

pub trait SharedIndexGetIndexUnsafe {
    /**
     * not directly unsafe, but still should be minimized.
     */
    fn get_index_unsafe(&self) -> u128;
}

impl SharedIndexGetIndexUnsafe for SharedIndex {
    fn get_index_unsafe(&self) -> u128 {
        *self.index
    }
}

impl Drop for SharedIndex {
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.index);
        if count == 1 {
            self.dealloc.destroy_from_index(*self.index);
        }
    }
}
