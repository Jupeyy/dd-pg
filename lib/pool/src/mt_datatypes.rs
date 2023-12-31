use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use hashlink::{LinkedHashMap, LinkedHashSet};

use crate::{mt_pool::Pool, mt_recycle::Recycle};

pub type PoolLinkedHashMap<K, V> = Recycle<LinkedHashMap<K, V>>;
pub type PoolLinkedHashSet<K> = Recycle<LinkedHashSet<K>>;
pub type PoolHashMap<K, V> = Recycle<HashMap<K, V>>;
pub type PoolHashSet<K> = Recycle<HashSet<K>>;

pub type PoolVec<T> = Recycle<Vec<T>>;
pub type PoolVecDeque<T> = Recycle<VecDeque<T>>;
pub type PoolBTreeMap<K, V> = Recycle<BTreeMap<K, V>>;

pub type PoolString = Recycle<String>;
pub type StringPool = Pool<String>;
