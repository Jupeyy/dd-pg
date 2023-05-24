use std::{cell::RefCell, hash::Hash, rc::Rc};

use crate::linked_list::{LinkedList, Node};

/**
 * This isn't really a data structure, but a helper structure that makes
 * the implementation behave like a hash map from lookup times
 * but also implements a queue to remember which
 * entry was inserted first.
 *
 * The struct guarantes the following:
 * - It has same add and remove, get performance like a HashMap
 * - It has the same lookup performance of the front item like a VecDeque
 */
pub struct HashQueue<K, V> {
    map: std::collections::HashMap<K, (Option<Rc<RefCell<Node<K>>>>, V)>,
    queue: LinkedList<K>,
    cur_queue_start_index: usize,
}

impl<K: Eq + Hash + Clone, V> HashQueue<K, V> {
    pub fn new() -> Self {
        Self {
            map: std::collections::HashMap::<K, (Option<Rc<RefCell<Node<K>>>>, V)>::new(),
            queue: LinkedList::<K>::new(),
            cur_queue_start_index: 0,
        }
    }

    /**
     * if the value sets, it will only overwrite the hash map entry
     * and not re-add it to the queue
     */
    pub fn add_or_set(&mut self, key: K, val: V) {
        let res = self.map.get_mut(&key);
        match res {
            None => {
                let node = self.queue.append(key.clone());
                self.map.insert(key, (node, val));
            }
            Some(val_hash) => {
                val_hash.1 = val;
            }
        }
    }

    fn pop_front(&mut self) {
        let front = self.queue.front();
        if let Some(front) = front {
            self.queue.rem(&mut Some(front.clone()));
        }
    }

    /**
     * Only return bool here to signal, if the value was removed,
     * direct access to the node is not really wanted
     */
    pub fn remove(&mut self, key: &K) -> bool {
        let entry_res = self.map.get_mut(&key);
        if let Some((queue_index, _entry)) = entry_res {
            self.queue.rem(queue_index);
            self.map.remove(key);
            return true;
        }
        false
    }

    pub fn front(&self) -> Option<&V> {
        let res = self.queue.front();
        if let Some(node) = res {
            let val = self.map.get(&node.borrow().data);
            if let Some((_, val)) = val {
                return Some(val);
            } else {
                return None;
            }
        }
        None
    }

    pub fn for_each_in_queue_order<T>(&self, mut predicate: T)
    where
        T: FnMut(&V),
    {
        let mut begin = self.queue.front().clone();
        while begin.is_some() {
            let item_res = self.map.get(&begin.as_ref().unwrap().borrow().data);
            if let Some(item) = item_res {
                predicate(&item.1);
            }
            let next = begin.as_ref().unwrap().borrow().next.clone();
            begin = next;
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        let res = self.map.get(k);
        if let Some((_, res)) = res {
            return Some(res);
        }
        None
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let res = self.map.get_mut(k);
        if let Some((_, res)) = res {
            return Some(res);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::hash_queue::HashQueue;

    #[test]
    fn it_works() {
        let mut hq = HashQueue::<String, String>::new();
        hq.add_or_set("1".to_string(), "1".to_string());
        hq.add_or_set("2".to_string(), "2".to_string());
        hq.add_or_set("3".to_string(), "3".to_string());
        hq.add_or_set("4".to_string(), "4".to_string());
        assert_eq!(hq.front().unwrap(), "1");
        hq.remove(&"1".to_string());
        assert_eq!(hq.front().unwrap(), "2");
        hq.remove(&"3".to_string());
        assert_eq!(hq.front().unwrap(), "2");
        hq.add_or_set("1".to_string(), "1".to_string());
        hq.add_or_set("3".to_string(), "3".to_string());
        assert_eq!(hq.front().unwrap(), "2");
        hq.remove(&"2".to_string());
        assert_eq!(hq.front().unwrap(), "4");
        hq.add_or_set("5".to_string(), "5".to_string());
        hq.add_or_set("6".to_string(), "6".to_string());
        hq.add_or_set("7".to_string(), "7".to_string());
        hq.add_or_set("8".to_string(), "8".to_string());
        assert_eq!(hq.front().unwrap(), "4");
        hq.remove(&"7".to_string());
        hq.remove(&"8".to_string());
        hq.remove(&"4".to_string());
        assert_eq!(hq.front().unwrap(), "1");
    }
}
