/**
 * This linked should only be used if you need direct access to the list nodes
 * Which LinkedList of std::collections does not provide
 */
use std::cell::RefCell;

use std::rc::{Rc, Weak};

pub struct Node<T> {
    pub data: T,
    pub prev: Option<Weak<RefCell<Node<T>>>>,
    pub next: Option<Rc<RefCell<Node<T>>>>,
}

impl<T> Node<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            prev: None,
            next: None,
        }
    }

    pub fn append(node: &mut Rc<RefCell<Node<T>>>, data: T) -> Option<Rc<RefCell<Node<T>>>> {
        let is_last = node.borrow().next.is_none();
        if is_last {
            let mut new_node = Node::new(data);
            new_node.prev = Some(Rc::downgrade(&node));
            let rc = Rc::new(RefCell::new(new_node));
            node.borrow_mut().next = Some(rc.clone());
            Some(rc)
        } else {
            if let Some(ref mut next) = node.borrow_mut().next {
                Self::append(next, data)
            } else {
                None
            }
        }
    }

    pub fn rem(node: &mut Rc<RefCell<Node<T>>>) {
        let mut prev = node.borrow_mut().prev.clone();
        let mut next = node.borrow_mut().next.clone();
        if let Some(ref mut prev) = prev {
            prev.upgrade().unwrap().borrow_mut().next = next.clone();
        }
        if let Some(ref mut next) = next {
            next.borrow_mut().prev = prev.clone();
        }
        node.borrow_mut().next = None;
        node.borrow_mut().prev = None;
    }
}

pub struct LinkedList<T> {
    first: Option<Rc<RefCell<Node<T>>>>,
    last: Option<Rc<RefCell<Node<T>>>>,
    size: usize,
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        Self {
            first: None,
            last: None,
            size: 0,
        }
    }

    pub fn append(&mut self, data: T) -> Option<Rc<RefCell<Node<T>>>> {
        self.size += 1;
        if let Some(ref mut next) = self.first {
            self.last = Node::append(next, data);
            return self.last.clone();
        } else {
            let f = Rc::new(RefCell::new(Node::new(data)));
            self.first = Some(f.clone());
            self.last = Some(f);
            return self.first.clone();
        }
    }

    pub fn rem(&mut self, node: &mut Option<Rc<RefCell<Node<T>>>>) {
        if let Some(ref mut node) = node {
            self.size -= 1;
            if node.borrow_mut().next.is_none() {
                if let Some(ref mut prev) = node.borrow_mut().prev {
                    self.last = prev.upgrade();
                } else {
                    self.last = None;
                    self.first = None;
                }
            } else if node.borrow_mut().prev.is_none() {
                if let Some(ref mut next) = node.borrow_mut().next {
                    self.first = Some(next.clone())
                } else {
                    self.last = None;
                    self.first = None;
                }
            }
            Node::rem(node);
        } else {
            panic!("node was invalid, please fix your implementation");
        }
    }

    pub fn size(&mut self) -> usize {
        self.size
    }

    pub fn front(&self) -> &Option<Rc<RefCell<Node<T>>>> {
        &self.first
    }
}
