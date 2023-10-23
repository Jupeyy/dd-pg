use std::rc::Rc;

pub trait SharedIndexCleanup {
    fn destroy_from_index(&self, index: u128);
}

/**
 * A helper struct for resources
 * that are shared.
 * E.g. textures, buffers etc.
 */
#[derive(Debug)]
pub struct SharedIndex<F>
where
    F: SharedIndexCleanup + ?Sized,
{
    index: Rc<u128>,
    dealloc: Rc<F>,
}

impl<F> Clone for SharedIndex<F>
where
    F: SharedIndexCleanup + ?Sized,
{
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            dealloc: self.dealloc.clone(),
        }
    }
}

impl<F> SharedIndex<F>
where
    F: SharedIndexCleanup + ?Sized,
{
    pub fn new(index: u128, dealloc: Rc<F>) -> Self {
        Self {
            index: Rc::new(index),
            dealloc,
        }
    }
}

impl<F> PartialEq for SharedIndex<F>
where
    F: SharedIndexCleanup + ?Sized,
{
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

impl<F> SharedIndexGetIndexUnsafe for SharedIndex<F>
where
    F: SharedIndexCleanup + ?Sized,
{
    fn get_index_unsafe(&self) -> u128 {
        *self.index
    }
}

impl<F> Drop for SharedIndex<F>
where
    F: SharedIndexCleanup + ?Sized,
{
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.index);
        if count == 1 {
            self.dealloc.destroy_from_index(*self.index);
        }
    }
}
