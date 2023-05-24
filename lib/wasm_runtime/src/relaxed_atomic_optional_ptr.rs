use std::sync::atomic::AtomicPtr;

pub struct RelaxedAtomicPtrOption<T> {
    ptr: AtomicPtr<T>,
}

impl<T> RelaxedAtomicPtrOption<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self {
            ptr: AtomicPtr::new(ptr),
        }
    }

    pub fn load(&self) -> Option<&mut T> {
        let ptr = self.ptr.load(std::sync::atomic::Ordering::Relaxed);
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *ptr })
        }
    }

    pub fn store(&self, ptr: *mut T) {
        self.ptr.store(ptr, std::sync::atomic::Ordering::Relaxed);
    }
}
