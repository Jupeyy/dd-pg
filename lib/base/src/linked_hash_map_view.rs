use std::borrow::Borrow;
use std::hash::Hash;

use hashlink::LinkedHashMap;

/// Create a view that only returns elements where
/// the filter function returns true
pub struct LinkedHashMapView<'a, K, V, F>
where
    F: Fn(&K) -> bool,
{
    hash_map: &'a mut LinkedHashMap<K, V>,
    key_filter_func: F,
}

impl<'a, K: Eq + Hash, V, F> LinkedHashMapView<'a, K, V, F>
where
    F: Fn(&K) -> bool,
{
    pub fn new(hash_map: &'a mut LinkedHashMap<K, V>, key_filter_func: F) -> Self {
        Self {
            hash_map,
            key_filter_func,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        if !(self.key_filter_func)(key.borrow()) {
            None
        } else {
            self.hash_map.get(key)
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if !(self.key_filter_func)(key.borrow()) {
            None
        } else {
            self.hash_map.get_mut(key)
        }
    }

    /// you know what you are doing
    pub fn into_inner(self) -> (&'a mut LinkedHashMap<K, V>, F) {
        (self.hash_map, self.key_filter_func)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.hash_map
            .iter()
            .filter(|(k, _)| (self.key_filter_func)(k))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.hash_map
            .iter_mut()
            .filter(|(k, _)| (self.key_filter_func)(k))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn into_iter(self) -> impl Iterator<Item = (&'a K, &'a mut V)> {
        self.hash_map
            .iter_mut()
            .filter(move |(k, _)| (self.key_filter_func)(k))
    }
}

pub struct LinkedHashMapExceptView<'a, K, V> {
    hash_map: &'a mut LinkedHashMap<K, V>,
    ignore_key: K,
}

impl<'a, K: Eq + Hash, V> LinkedHashMapExceptView<'a, K, V> {
    pub fn new(hash_map: &'a mut LinkedHashMap<K, V>, ignore_key: K) -> Self {
        Self {
            hash_map,
            ignore_key,
        }
    }
}

impl<'a, K: Eq + Hash, V> LinkedHashMapExceptView<'a, K, V> {
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.ignore_key.borrow().eq(key) {
            None
        } else {
            self.hash_map.get(key)
        }
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.ignore_key.borrow().eq(key) {
            None
        } else {
            self.hash_map.get_mut(key)
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.hash_map.iter().filter(|(k, _)| **k != self.ignore_key)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.hash_map
            .iter_mut()
            .filter(|(k, _)| **k != self.ignore_key)
    }
}

pub struct LinkedHashMapIterExt<'a, K, V> {
    hash_map: &'a mut LinkedHashMap<K, V>,
    rev: bool,
}

impl<'a, K, V> LinkedHashMapIterExt<'a, K, V> {
    pub fn new(hash_map: &'a mut LinkedHashMap<K, V>) -> Self {
        Self {
            hash_map,
            rev: false,
        }
    }
}

impl<'a, K: Eq + Hash + Copy + Clone + 'static, V: 'static> LinkedHashMapIterExt<'a, K, V> {
    pub fn rev(self) -> Self {
        Self {
            hash_map: self.hash_map,
            rev: !self.rev,
        }
    }

    pub fn for_each<F>(&'a mut self, f: F)
    where
        F: FnMut((&'a K, (&'a mut V, LinkedHashMapExceptView<'a, K, V>))),
    {
        let hash_map_ptr: *mut LinkedHashMap<K, V> = self.hash_map;
        fn it_for_each<'a, K, V: 'a, I, F>(it: I, mut f: F, hash_map_ptr: *mut LinkedHashMap<K, V>)
        where
            K: 'a + Eq + Hash + Copy + Clone,
            I: Iterator<Item = (&'a K, &'a mut V)>,
            F: FnMut((&'a K, (&'a mut V, LinkedHashMapExceptView<'a, K, V>))),
        {
            for (k, v) in it {
                f((
                    k,
                    (
                        v,
                        LinkedHashMapExceptView::new(unsafe { &mut *hash_map_ptr }, *k),
                    ),
                ));
            }
        }

        if self.rev {
            it_for_each(self.hash_map.iter_mut().rev(), f, hash_map_ptr);
        } else {
            it_for_each(self.hash_map.iter_mut(), f, hash_map_ptr);
        }
    }
}

pub struct LinkedHashMapEntryAndRes {}

impl LinkedHashMapEntryAndRes {
    pub fn get<'a, K: Eq + Hash + Copy + Clone + 'static, V: 'static>(
        hash_map: &'a mut LinkedHashMap<K, V>,
        key: &K,
    ) -> (&'a mut V, LinkedHashMapExceptView<'a, K, V>) {
        let hash_map_ptr: *mut LinkedHashMap<K, V> = hash_map;

        let res = hash_map.get_mut(key).unwrap();

        (
            res,
            LinkedHashMapExceptView::new(unsafe { &mut *hash_map_ptr }, *key),
        )
    }
}
