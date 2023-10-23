use std::collections::{HashMap, HashSet, VecDeque};

use hashlink::{LinkedHashMap, LinkedHashSet};

use crate::{recycle::Recycle, pool::Pool};

pub type PoolLinkedHashMap<K, V> = Recycle<LinkedHashMap<K, V>>;
pub type PoolLinkedHashSet<K> = Recycle<LinkedHashSet<K>>;
pub type PoolHashMap<K, V> = Recycle<HashMap<K, V>>;
pub type PoolHashSet<K> = Recycle<HashSet<K>>;

pub type PoolVec<T> = Recycle<Vec<T>>;
pub type PoolVecDeque<T> = Recycle<VecDeque<T>>;

pub type PoolString = Recycle<String>;
pub type StringPool = Pool<String>;
