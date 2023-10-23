#[cfg(debug_assertions)]
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

#[cfg(debug_assertions)]
use hashlink::LinkedHashMap;

/**
 * A counted index makes sure that
 * the underlaying index is not dropped
 * without being passed to a deallocation
 * implementation.
 * This can be useful for resources like
 * textures that are too complex to hold
 * references to their owners in order
 * to do automatic cleanup.
 * `IMPL_HOLDS_A_REFERENCE` signals the counting reference that the implementation itself
 * also holds a reference => the last valid reference outside of the implementation is not the
 * last reference at all.
 * It only checks the references in debug mode!
 */
#[derive(Debug, PartialEq)]
pub struct CountedIndex<const IMPL_HOLDS_A_REFERENCE: bool> {
    #[cfg(debug_assertions)]
    index: Rc<(usize, RefCell<(bool, LinkedHashMap<usize, String>)>)>,
    #[cfg(debug_assertions)]
    dbg_index: usize,
    #[cfg(not(debug_assertions))]
    index: usize,
}

impl<const IMPL_HOLDS_A_REFERENCE: bool> Clone for CountedIndex<IMPL_HOLDS_A_REFERENCE> {
    #[cfg(debug_assertions)]
    fn clone(&self) -> Self {
        let mut hash_map_bor = self.index.1.borrow_mut();
        let hash_map = &mut hash_map_bor.1;
        let dbg_index = hash_map.back().map_or(0, |d| d.0 + 1);
        hash_map.insert(
            dbg_index,
            std::backtrace::Backtrace::force_capture().to_string(),
        );
        drop(hash_map_bor);
        Self {
            index: Rc::clone(&self.index),
            dbg_index: dbg_index,
        }
    }

    #[cfg(not(debug_assertions))]
    fn clone(&self) -> Self {
        Self { index: self.index }
    }
}

#[derive(Debug)]
pub struct ScopedCountedIndex {
    #[cfg(debug_assertions)]
    index: Weak<(usize, RefCell<(bool, LinkedHashMap<usize, String>)>)>,
    #[cfg(not(debug_assertions))]
    index: usize,
}

impl PartialEq for ScopedCountedIndex {
    #[cfg(debug_assertions)]
    fn eq(&self, other: &Self) -> bool {
        self.index.ptr_eq(&other.index)
    }
    #[cfg(not(debug_assertions))]
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

pub trait CountedIndexDrop {
    /**
     * not directly unsafe, this should only be called
     * by the implementation freeing the index.
     * asserts, if there are still valid references to this object.
     * It makes sure that this is the last index. (ignoring the reference the implementation can hold)
     * This should never be called on the reference the implementation might hold
     */
    fn drop_index_without_logic_unsafe(self);
}

pub trait CountedIndexGetIndexUnsafe {
    /**
     * not directly unsafe, but still should be minimized.
     */
    fn get_index_unsafe(&self) -> usize;
}

impl CountedIndexGetIndexUnsafe for ScopedCountedIndex {
    #[cfg(debug_assertions)]
    fn get_index_unsafe(&self) -> usize {
        self.index.upgrade().unwrap().0
    }
    #[cfg(not(debug_assertions))]
    fn get_index_unsafe(&self) -> usize {
        self.index
    }
}

impl<const IMPL_HOLDS_A_REFERENCE: bool> CountedIndex<IMPL_HOLDS_A_REFERENCE> {
    pub fn new(index: usize) -> Self {
        #[cfg(debug_assertions)]
        let mut hash_map: LinkedHashMap<usize, String> = Default::default();
        #[cfg(debug_assertions)]
        hash_map.insert(0, "".to_string());
        Self {
            #[cfg(debug_assertions)]
            index: Rc::new((index, RefCell::new((true, hash_map)))),
            #[cfg(debug_assertions)]
            dbg_index: 0,
            #[cfg(not(debug_assertions))]
            index,
        }
    }

    /**
     * Get a temporary scoped version of this index
     */
    #[cfg(debug_assertions)]
    pub fn as_temp(&self) -> ScopedCountedIndex {
        ScopedCountedIndex {
            index: Rc::downgrade(&self.index),
        }
    }
    #[cfg(not(debug_assertions))]
    pub fn as_temp(&self) -> ScopedCountedIndex {
        ScopedCountedIndex { index: self.index }
    }
}

impl<const IMPL_HOLDS_A_REFERENCE: bool> CountedIndexGetIndexUnsafe
    for CountedIndex<IMPL_HOLDS_A_REFERENCE>
{
    #[cfg(debug_assertions)]
    fn get_index_unsafe(&self) -> usize {
        self.index.0
    }
    #[cfg(not(debug_assertions))]
    fn get_index_unsafe(&self) -> usize {
        self.index
    }
}

impl CountedIndexDrop for CountedIndex<true> {
    #[cfg(debug_assertions)]
    fn drop_index_without_logic_unsafe(self) {
        let count = Rc::strong_count(&self.index);
        assert!(
            count <= 2,
            "dropped an index that was still in use. In-use count was: {}",
            count
        );
        let i = Rc::downgrade(&self.index);
        self.index.1.borrow_mut().0 = false;
        drop(self);
        i.upgrade().unwrap().1.borrow_mut().0 = true;
    }
    #[cfg(not(debug_assertions))]
    fn drop_index_without_logic_unsafe(self) {}
}

impl CountedIndexDrop for CountedIndex<false> {
    #[cfg(debug_assertions)]
    fn drop_index_without_logic_unsafe(self) {
        let count = Rc::strong_count(&self.index);
        assert!(
            count <= 1,
            "dropped an index that was still in use. In-use count was: {}",
            count
        );
        self.index.1.borrow_mut().0 = false;
        drop(self);
    }
    #[cfg(not(debug_assertions))]
    fn drop_index_without_logic_unsafe(self) {}
}

impl<const IMPL_HOLDS_A_REFERENCE: bool> Drop for CountedIndex<IMPL_HOLDS_A_REFERENCE> {
    #[cfg(debug_assertions)]
    fn drop(&mut self) {
        let mut info = self.index.1.borrow_mut();
        if info.0 {
            let count = Rc::strong_count(&self.index);
            let is_valid = (IMPL_HOLDS_A_REFERENCE && count == 1)
                || (count > if IMPL_HOLDS_A_REFERENCE { 2 } else { 1 });
            // print stack traces
            if !is_valid {
                println!("The resource was still in used, by these calls:");
                info.1.iter().for_each(|(_, bt)| println!("{}", bt));
            }
            // there must be more indices than the last index + the index hold by the implementation (if any)
            // assert, since this is a fatal error
            assert!(
                is_valid,
                "dropped an index without passing it to the corresponding uninitilization implementation. This means a memory leak. In-use count was: {}",
                count
            );
        }
        info.1.remove(&self.dbg_index);
    }
    #[cfg(not(debug_assertions))]
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use crate::counted_index::CountedIndex;
    use crate::counted_index::CountedIndexDrop;

    #[test]
    fn it_works() {
        // for non holding reference
        let index = CountedIndex::<false>::new(1);
        let index_clone = index.clone();
        let index_clone2 = index.clone();
        drop(index_clone2);
        drop(index_clone);
        index.drop_index_without_logic_unsafe();

        // for holding reference
        let index = CountedIndex::<true>::new(1);
        let index_clone = index.clone();
        let index_clone2 = index.clone();
        drop(index_clone2);
        // this is the last index not used by the implementation, the function must be called on it
        index_clone.drop_index_without_logic_unsafe();
        // the reference of the implementation can be free'd without a problem
        drop(index);
    }

    /* TODO: test cannot unwind, because it happens inside a Drop
    #[test]
    #[should_panic]
    fn ensures_panic() {
        // for non holding reference
        let index = CountedIndex::<false>::new(1);
        let index_clone = index.clone();
        let index_clone2 = index.clone();
        drop(index_clone2);
        index.drop_index_without_logic_unsafe();
        // a reference was still in use
        drop(index_clone);
    }
    */

    #[test]
    #[should_panic]
    fn ensures_panic2() {
        // for holding reference
        let index = CountedIndex::<true>::new(1);
        let index_clone = index.clone();
        let index_clone2 = index.clone();
        drop(index_clone2);
        // dropped the reference of the implementation before the last index was dropped
        drop(index);
        index_clone.drop_index_without_logic_unsafe();
    }
}
