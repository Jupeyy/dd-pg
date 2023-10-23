use std::collections::{HashMap, HashSet, VecDeque};

use hashlink::{LinkedHashMap, LinkedHashSet};

pub trait Recyclable {
    fn new() -> Self;
    fn reset(&mut self);
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
