use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use hashlink::{LinkedHashMap, LinkedHashSet};
use rustc_hash::{FxHashMap, FxHashSet};

pub trait Recyclable {
    fn new() -> Self;
    fn reset(&mut self);
    fn should_put_to_pool(&self) -> bool {
        true
    }
}

// # vec
impl<T> Recyclable for Vec<T> {
    fn new() -> Self {
        Vec::<T>::default()
    }

    fn reset(&mut self) {
        self.clear();
    }
}

// # vec deque
impl<T> Recyclable for VecDeque<T> {
    fn new() -> Self {
        VecDeque::<T>::default()
    }

    fn reset(&mut self) {
        self.clear();
    }
}

// # btree map
impl<K, V> Recyclable for BTreeMap<K, V> {
    fn new() -> Self {
        BTreeMap::<K, V>::default()
    }

    fn reset(&mut self) {
        self.clear();
    }
}

// # btree set
impl<K> Recyclable for BTreeSet<K> {
    fn new() -> Self {
        BTreeSet::<K>::default()
    }

    fn reset(&mut self) {
        self.clear();
    }
}

// # linked hash map
impl<K, V> Recyclable for LinkedHashMap<K, V> {
    fn new() -> Self {
        Default::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

// # linked hash set
impl<K> Recyclable for LinkedHashSet<K> {
    fn new() -> Self {
        Default::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

// # fx hash map
impl<K, V> Recyclable for FxHashMap<K, V> {
    fn new() -> Self {
        Default::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

// # fx hash set
impl<K> Recyclable for FxHashSet<K> {
    fn new() -> Self {
        Default::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

// # hash map
impl<K, V> Recyclable for HashMap<K, V> {
    fn new() -> Self {
        Default::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

// # hash set
impl<K> Recyclable for HashSet<K> {
    fn new() -> Self {
        Default::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

impl Recyclable for String {
    fn new() -> Self {
        String::default()
    }

    fn reset(&mut self) {
        self.clear()
    }
}

// # cow
impl<'a, T> Recyclable for std::borrow::Cow<'a, [T]>
where
    [T]: ToOwned<Owned: Default + Recyclable>,
{
    fn new() -> Self {
        std::borrow::Cow::<'a, [T]>::default()
    }

    fn reset(&mut self) {
        self.to_mut().reset();
    }
}

// # Box, note that they create heap allocations
impl<T: Recyclable> Recyclable for Box<T> {
    fn new() -> Self {
        Box::new(T::new())
    }

    fn reset(&mut self) {
        self.as_mut().reset();
    }
}
