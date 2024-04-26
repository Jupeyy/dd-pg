use std::fmt::Debug;

use crate::{
    mt_pool::Pool as MtPool, mt_recycle::Recycle as MtRecycle, pool::Pool as StPool,
    traits::Recyclable,
};

/// Call [`PoolSyncPoint::sync`] at least once in your app per iteration (e.g. a game loop).
pub trait PoolSyncPoint: Debug {
    fn sync(&self);
}

/// the mixed pool is a combination of the single threaded pool & the multi threaded pool.
/// the idea is that if you always allocate from the pool in one thread, but share your
/// allocations with other threads, you might also have a sync point which is a perfect spot
/// to put all in threaded used allocations back to the single threaded pool (because contention is unlikely).
/// This sync point is expressed by the [`PoolSyncPoint`] trait, which this pool implements.
///
/// # Note
///
/// If you don't call the [`PoolSyncPoint::sync`] function anywhere in your program, this will lead
/// to normal allocations at some point and leaks them, because they are never pushed back to the single
/// threaded pool.
///
/// # Example
///
/// Consider you are writing a single threaded app, but want to push your network events to a
/// network thread to offload some heavy workloads:
///
/// ```rust
/// use pool::mixed_pool::Pool;
/// use pool::mixed_pool::PoolSyncPoint;
/// use pool::mt_datatypes::PoolVec;
/// pub enum Event {
///     SomethingHappens,
/// }
///
/// pub struct Network {
///     evs_to_handle: std::sync::Arc<std::sync::Mutex<PoolVec<Event>>>,
///     sync_point: Box<dyn PoolSyncPoint>,
///     // [..] threads etc.
/// }
///
/// impl Network {
///     pub fn handle_events<F>(&self, evs: PoolVec<Event>, f: F)
///     where F: FnOnce()
///     {
///         let mut g = self.evs_to_handle.lock().unwrap();
///         *g = evs;
///         // this call is just for showing purpose
///         f();
///         // HERE is the important sync point.. inside the lock,
///         // so the network thread is probably not in use right now
///         // so it probably also does not drop any pool allocated objects
///         // (at least let's assume the thread was written like that)
///         self.sync_point.sync();
///         drop(g);
///         // do whatever inside a thread
///     }
/// }
///
/// pub struct Client {
///     network: Network,
///     pool: Pool<Vec<Event>>,
/// }
///
/// impl Client {
///     pub fn run(&self) {
///         // items in pool are zero, because there were no pool objects pushed back to the pool yet.
///         assert!(self.pool.items_in_pool() == 0);
///         // there are no items waiting for a sync point
///         assert!(self.pool.items_waiting_for_sync() == 0);
///         let mut evs = self.pool.new();
///         evs.push(Event::SomethingHappens);
///         self.network.handle_events(evs, || {
///             // right before the sync point there is exactly one item in the mt pool
///             // the one droped in the above move assignment
///             assert!(self.pool.items_waiting_for_sync() == 1);
///         });
///         // in the main function the network already got one pool object,
///         // this is the one that should have been pushed here
///         assert!(self.pool.items_in_pool() == 1);
///     }
/// }
///
/// fn main() {
///     let pool = Pool::with_capacity(64);
///     let client = Client {
///         network: Network {
///             evs_to_handle: std::sync::Arc::new(std::sync::Mutex::new(pool.new())),
///             sync_point: Box::new(pool.clone()),
///         },
///         pool,
///     };
///     client.run();
/// }
/// ```
#[cfg_attr(feature = "enable_hiarc", derive(hiarc::Hiarc))]
#[derive(Debug)]
pub struct Pool<T: Recyclable + Send> {
    mt_pool: MtPool<T>,
    st_pool: StPool<T>,
}

impl<T: Debug + Recyclable + Send> PoolSyncPoint for Pool<T> {
    fn sync(&self) {
        if !self.mt_pool.pool.is_empty() {
            let mut pool = self.mt_pool.pool.take();
            self.st_pool.pool.borrow_mut().append(&mut pool);
        }
    }
}

impl<T: Debug + Recyclable + Send + 'static> Pool<T> {
    /// If capacity is 0, the pool will not allocate memory for any elements, but will still create heap memory.
    pub fn with_capacity(capacity: usize) -> (Self, Box<dyn PoolSyncPoint>) {
        let res = Self {
            mt_pool: MtPool::with_capacity(capacity),
            st_pool: StPool::with_capacity(capacity),
        };

        let sync_point = Box::new(res.clone());
        (res, sync_point)
    }

    pub fn with_sized<F>(new_size: usize, item_constructor: F) -> (Self, Box<dyn PoolSyncPoint>)
    where
        F: FnMut() -> T,
    {
        let res = Self {
            mt_pool: MtPool::with_capacity(new_size),
            st_pool: StPool::with_sized(new_size, item_constructor),
        };

        let sync_point = Box::new(res.clone());
        (res, sync_point)
    }

    pub fn new(&self) -> MtRecycle<T> {
        let res = self.st_pool.new();
        let inner = res.take();
        MtRecycle::new_with_pool(inner, self.mt_pool.pool.clone())
    }

    pub fn items_in_pool(&self) -> usize {
        self.st_pool.items_in_pool()
    }

    /// this function will cause a lock and thus is slow, use with care.
    pub fn items_waiting_for_sync(&self) -> usize {
        self.mt_pool.items_in_pool()
    }
}

impl<T: Recyclable + Send> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Self {
            mt_pool: self.mt_pool.clone(),
            st_pool: self.st_pool.clone(),
        }
    }
}
