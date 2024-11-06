use std::thread::JoinHandle;

use hiarc::Hiarc;

/// A thread that joins when dropped.
/// It should _usually_ be the last member of a struct
#[derive(Debug, Hiarc)]
pub struct JoinThread<T>(Option<JoinHandle<T>>);

impl<T> JoinThread<T> {
    pub const fn new(handle: JoinHandle<T>) -> Self {
        Self(Some(handle))
    }
    pub const fn new_opt(handle: Option<JoinHandle<T>>) -> Self {
        Self(handle)
    }
}

impl<T> Drop for JoinThread<T> {
    fn drop(&mut self) {
        if let Some(thread) = self.0.take() {
            let _ = thread.join();
        }
    }
}
