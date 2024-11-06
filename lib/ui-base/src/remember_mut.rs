use std::ops::{Deref, DerefMut};

use hiarc::Hiarc;

/// A small generic wrapper that remembers
/// if the inner value was accessed mutable.
/// If that is the case this access can be queried.
#[derive(Debug, Hiarc, Default)]
pub struct RememberMut<T> {
    val: T,
    was_accessed_mut: bool,
}

impl<T> RememberMut<T> {
    pub fn new(val: T) -> Self {
        Self {
            val,
            was_accessed_mut: false,
        }
    }

    /// Query if this object was acccessed mutable.
    /// Resets the the tracker in this call.
    pub fn was_accessed_mut(&mut self) -> bool {
        std::mem::take(&mut self.was_accessed_mut)
    }
}

impl<T> Deref for RememberMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> DerefMut for RememberMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.was_accessed_mut = true;
        &mut self.val
    }
}

impl<T> From<T> for RememberMut<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
