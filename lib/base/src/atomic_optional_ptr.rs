use std::sync::atomic::AtomicPtr;

pub struct AtomicPtrOption<T> {
    ptr: AtomicPtr<T>,
}

impl<T> AtomicPtrOption<T> {
    pub const fn new(ptr: *mut T) -> Self {
        Self {
            ptr: AtomicPtr::new(ptr),
        }
    }

    pub fn load(&self) -> Option<&mut T> {
        let ptr = self.ptr.load(std::sync::atomic::Ordering::SeqCst);
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *ptr })
        }
    }

    pub fn store(&self, ptr: *mut T) {
        self.ptr.store(ptr, std::sync::atomic::Ordering::SeqCst);
    }
}

impl<T> Default for AtomicPtrOption<T> {
    fn default() -> Self {
        Self {
            ptr: AtomicPtr::default(),
        }
    }
}
