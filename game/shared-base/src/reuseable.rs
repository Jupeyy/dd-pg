use std::collections::{HashMap, HashSet};

use pool::traits::Recyclable;

/// guarantees that the underlaying elements are copyable,
/// besides that it's simply a `clone_from`
pub trait CloneWithCopyableElements {
    fn copy_clone_from(&mut self, other: &Self);
}

impl<T> CloneWithCopyableElements for Vec<T>
where
    T: Default + Copy + Clone,
{
    fn copy_clone_from(&mut self, other: &Self) {
        self.clone_from(other);
    }
}

impl<K, V> CloneWithCopyableElements for HashMap<K, V>
where
    K: Default + Copy + Clone,
    V: Default + Copy + Clone,
{
    fn copy_clone_from(&mut self, other: &Self) {
        self.clone_from(other);
    }
}

impl<K> CloneWithCopyableElements for HashSet<K>
where
    K: Default + Copy + Clone,
{
    fn copy_clone_from(&mut self, other: &Self) {
        self.clone_from(other);
    }
}

pub trait ReusableCore: CloneWithCopyableElements + Recyclable {}
