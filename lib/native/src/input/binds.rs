use std::{collections::BTreeSet, iter::Peekable};

use hashlink::LinkedHashMap;
use winit::{event::MouseButton, keyboard::KeyCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseExtra {
    WheelDown,
    WheelUp,
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindKey {
    Key(KeyCode),
    Mouse(MouseButton),
    Extra(MouseExtra),
}

#[derive(Clone)]
pub enum BindTarget<F> {
    Scancode(KeyTarget<F>),
    Funcs(Vec<F>),
    ScancodeAndFuncs((KeyTarget<F>, Vec<F>)),
}

pub type KeyTarget<F> = LinkedHashMap<BindKey, BindTarget<F>>;

pub struct Binds<F> {
    keys: KeyTarget<F>,
    cur_keys_pressed_is_order: BTreeSet<BindKey>,
    consumed: bool,

    helper_vec: Vec<F>,
}

impl<F> Default for Binds<F> {
    fn default() -> Self {
        Self {
            keys: Default::default(),
            cur_keys_pressed_is_order: Default::default(),
            consumed: Default::default(),
            helper_vec: Default::default(),
        }
    }
}

impl<F: Clone> Binds<F> {
    pub fn handle_key_down(&mut self, code: &BindKey) {
        self.cur_keys_pressed_is_order.insert(*code);
        self.consumed = false;
    }

    pub fn handle_key_up(&mut self, code: &BindKey) {
        self.cur_keys_pressed_is_order.remove(code);
        self.consumed = false;
    }

    pub fn clear_pressed_keys(&mut self) {
        self.cur_keys_pressed_is_order.clear();
        self.consumed = false;
    }

    pub fn process(&mut self, consume_event_until_change: bool) -> Option<&Vec<F>> {
        if consume_event_until_change && self.consumed {
            None
        } else {
            // tries to find the bind with the longest chain possible
            // the first key(s) can be ignored (`can_ignore_keys`), because it might not have any bind at all
            fn find_longest_chain_func<'a, F>(
                mut key_iter: std::collections::btree_set::Iter<'a, BindKey>,
                keys: &'a KeyTarget<F>,
                can_ignore_keys: bool,
            ) -> Option<(&'a Vec<F>, std::collections::btree_set::Iter<'a, BindKey>)> {
                match key_iter.next() {
                    Some(next_key) => match keys.get(next_key) {
                        Some(key_binds) => match key_binds {
                            BindTarget::Scancode(cur_scan) => {
                                find_longest_chain_func(key_iter, cur_scan, false)
                            }
                            BindTarget::Funcs(funcs) => Some((funcs, key_iter)),
                            BindTarget::ScancodeAndFuncs((cur_scan, funcs)) => {
                                let res =
                                    find_longest_chain_func(key_iter.clone(), cur_scan, false);
                                // prefer longest chain if available
                                if res.is_some() {
                                    res
                                } else {
                                    Some((funcs, key_iter))
                                }
                            }
                        },
                        // if nothing was found at this key, try the
                        None => {
                            if can_ignore_keys {
                                find_longest_chain_func(key_iter, keys, true)
                            } else {
                                None
                            }
                        }
                    },
                    None => None,
                }
            }

            self.helper_vec.clear();
            let mut key_iter = self.cur_keys_pressed_is_order.iter();
            while let Some((funcs, key_iter_next)) =
                find_longest_chain_func(key_iter, &self.keys, true)
            {
                key_iter = key_iter_next;
                funcs.iter().for_each(|f| self.helper_vec.push(f.clone()));
            }
            if self.helper_vec.len() > 0 {
                Some(&self.helper_vec)
            } else {
                None
            }
        }
    }

    pub fn register_bind(&mut self, bind_keys: &[BindKey], func: F) {
        let keys = &mut self.keys;

        fn insert_into_keys<F: Clone>(
            mut key_iter: Peekable<std::collections::btree_set::Iter<'_, BindKey>>,
            keys: &mut KeyTarget<F>,
            func: F,
        ) {
            match key_iter.next() {
                Some(scancode) => {
                    if key_iter.peek().is_some() {
                        if let Some(cur) = keys.get_mut(scancode) {
                            match cur {
                                BindTarget::Scancode(cur_scan) => {
                                    insert_into_keys(key_iter, cur_scan, func)
                                }
                                BindTarget::Funcs(cur_func) => {
                                    let repl_func = cur_func.clone();
                                    *cur = BindTarget::ScancodeAndFuncs((
                                        Default::default(),
                                        repl_func,
                                    ));
                                }
                                BindTarget::ScancodeAndFuncs((cur_scan, _)) => {
                                    insert_into_keys(key_iter, cur_scan, func)
                                }
                            }
                        } else {
                            keys.insert(*scancode, BindTarget::Scancode(Default::default()));
                        }
                    } else {
                        if let Some(cur) = keys.get_mut(scancode) {
                            match cur {
                                BindTarget::Scancode(cur_scan) => {
                                    let repl_scan = cur_scan.clone();
                                    *cur = BindTarget::ScancodeAndFuncs((repl_scan, vec![func]))
                                }
                                BindTarget::Funcs(funcs) => funcs.push(func),
                                BindTarget::ScancodeAndFuncs((_, funcs)) => funcs.push(func),
                            }
                        } else {
                            keys.insert(*scancode, BindTarget::Funcs(vec![func]));
                        }
                    }
                }
                None => {}
            }
        }
        let keys_in_order: BTreeSet<BindKey> = bind_keys
            .iter()
            .map(|key| *key)
            .collect::<BTreeSet<BindKey>>();
        insert_into_keys(keys_in_order.iter().peekable(), keys, func);
    }
}
