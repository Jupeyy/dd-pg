use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::anyhow;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct TokIOBatcherTask<S> {
    pub queue_id: u64,
    storage_task: Arc<tokio::sync::Mutex<anyhow::Result<S>>>,
    is_finished: Arc<AtomicBool>,

    // private
    io_batcher: Rc<RefCell<TokIOBatcherInner>>,
}

impl<S> TokIOBatcherTask<S> {
    fn wait_finished_and_drop(&mut self) {
        let mut inner = self.io_batcher.borrow_mut();
        let task_join = inner.tasks.remove(&self.queue_id).unwrap();
        let _g = inner.rt.enter();
        tokio::task::block_in_place(|| {
            inner.rt.block_on(async move { task_join.await }).unwrap();
        });
    }

    pub fn get_storage(mut self) -> anyhow::Result<S> {
        self.wait_finished_and_drop();
        let mut storage_res = Err(anyhow!("not started yet"));
        std::mem::swap(&mut *self.storage_task.blocking_lock(), &mut storage_res);
        storage_res
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Debug)]
struct TokIOBatcherInner {
    tasks: HashMap<u64, JoinHandle<()>>,
    rt: tokio::runtime::Runtime,
}

#[derive(Debug)]
pub struct TokIOBatcher {
    inner: Rc<RefCell<TokIOBatcherInner>>,
    task_id: Rc<RefCell<u64>>,
}

impl TokIOBatcher {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        Self {
            inner: Rc::new(RefCell::new(TokIOBatcherInner {
                tasks: HashMap::new(),
                rt: rt,
            })),
            task_id: Default::default(),
        }
    }

    /**
     * If you are sure you want to create a task without pushing it to the internal queue,
     * you can do so:
     * Pros:
     *  - The task is fully controlled by the called of this function, the io_batcher does not need to
     *      be notified or asked about it
     * Cons:
     *  - You cannot wait for the task to be finished
     *  - The lifetime of this object might exceed the callers lifetime
     */
    pub fn spawn_without_queue<S: Send + Sync + 'static, F>(
        &self,
        task: F,
    ) -> (TokIOBatcherTask<S>, JoinHandle<()>)
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let id = u64::MAX;

        let storage_task = Arc::new(tokio::sync::Mutex::new(Err(anyhow!("not started yet"))));
        let storage_task_clone = storage_task.clone();

        let task_finished = Arc::new(AtomicBool::new(false));
        let task_finished_clone = task_finished.clone();

        let _g = self.inner.borrow_mut().rt.enter();
        let join_handle = tokio::spawn(async move {
            let storage_wrapped = task.await;
            *storage_task.lock().await = storage_wrapped;
            task_finished_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        (
            TokIOBatcherTask::<S> {
                queue_id: id,
                storage_task: storage_task_clone,
                is_finished: task_finished,
                io_batcher: self.inner.clone(),
            },
            join_handle,
        )
    }

    pub fn spawn<S: Send + Sync + 'static, F>(&self, task: F) -> TokIOBatcherTask<S>
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let mut task = self.spawn_without_queue(task);
        task.0.queue_id = *self.task_id.borrow();
        *self.task_id.borrow_mut() += 1;

        self.inner
            .borrow_mut()
            .tasks
            .insert(task.0.queue_id, task.1);

        task.0
    }
}

impl Drop for TokIOBatcher {
    fn drop(&mut self) {
        let mut inner = self.inner.borrow_mut();
        let mut tasks = Default::default();
        std::mem::swap(&mut tasks, &mut inner.tasks);
        let _g = inner.rt.enter();
        for (_, task) in tasks.drain() {
            tokio::task::block_in_place(|| {
                inner.rt.block_on(async move { task.await }).unwrap();
            });
        }
    }
}
