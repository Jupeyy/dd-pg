use std::borrow::Borrow;
use std::hash::Hash;

use hashlink::LinkedHashMap;

pub struct LinkedHashMapView<'a, K, V> {
    hash_map: &'a mut LinkedHashMap<K, V>,
    ignore_key: K,
}

impl<'a, K: Eq + Hash, V> LinkedHashMapView<'a, K, V> {
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
}

impl<'a, K, V> LinkedHashMapIterExt<'a, K, V> {
    pub fn new(hash_map: &'a mut LinkedHashMap<K, V>) -> Self {
        Self { hash_map }
    }
}

impl<'a, K: Copy + Clone + 'static, V: 'static> LinkedHashMapIterExt<'a, K, V> {
    pub fn for_each<F>(&'a mut self, mut f: F)
    where
        F: FnMut((&'a K, (&'a mut V, LinkedHashMapView<'a, K, V>))),
    {
        let hash_map_ptr: *mut LinkedHashMap<K, V> = self.hash_map;
        let it = self.hash_map.iter_mut();
        for (k, v) in it {
            f((
                k,
                (
                    v,
                    LinkedHashMapView {
                        hash_map: unsafe { &mut *hash_map_ptr },
                        ignore_key: *k,
                    },
                ),
            ));
        }
    }
}
