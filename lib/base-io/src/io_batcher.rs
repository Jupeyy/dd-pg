use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::anyhow;

#[cfg(not(target_arch = "wasm32"))]
pub type RuntimeType = tokio::runtime::Runtime;
#[cfg(target_arch = "wasm32")]
pub type RuntimeType = async_executor::LocalExecutor<'static>;

#[cfg(not(target_arch = "wasm32"))]
pub type TaskJoinType = tokio::task::JoinHandle<()>;
#[cfg(target_arch = "wasm32")]
pub type TaskJoinType = async_task::Task<()>;

#[derive(Debug)]
pub struct TokIOBatcherTask<S> {
    pub queue_id: u64,
    storage_task: Arc<tokio::sync::Mutex<anyhow::Result<S>>>,
    is_finished: Arc<AtomicBool>,

    // private
    io_batcher: Rc<RefCell<TokIOBatcherInner>>,
}

impl<S> TokIOBatcherTask<S> {
    fn wait_finished_and_drop(&self) {
        let mut inner = self.io_batcher.borrow_mut();
        let task_join = inner
            .tasks
            .remove(&self.queue_id)
            .ok_or_else(|| {
                anyhow!(
                    "Could not find queue id {} in {:?}",
                    self.queue_id,
                    inner.tasks
                )
            })
            .unwrap();
        #[cfg(not(target_arch = "wasm32"))]
        inner.rt.block_on(task_join).unwrap();
        #[cfg(target_arch = "wasm32")]
        futures_lite::future::block_on(inner.rt.run(task_join));
    }

    pub fn get_storage(self) -> anyhow::Result<S> {
        self.wait_finished_and_drop();
        let mut storage_res = Err(anyhow!("not started yet"));
        std::mem::swap(&mut *self.storage_task.blocking_lock(), &mut storage_res);
        storage_res
    }

    #[cfg(target_arch = "wasm32")]
    fn try_run(&self) {
        let mut inner = self.io_batcher.borrow_mut();
        inner.rt.try_tick();
    }

    pub fn is_finished(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        self.try_run();
        self.is_finished.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Debug)]
struct TokIOBatcherInner {
    tasks: HashMap<u64, TaskJoinType>,
    rt: RuntimeType,
}

impl Drop for TokIOBatcherInner {
    fn drop(&mut self) {
        let mut tasks = Default::default();
        std::mem::swap(&mut tasks, &mut self.tasks);
        for (_, task) in tasks.drain() {
            #[cfg(not(target_arch = "wasm32"))]
            self.rt.block_on(task).unwrap();
            #[cfg(target_arch = "wasm32")]
            self.rt.run(task);
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokIOBatcher {
    inner: Rc<RefCell<TokIOBatcherInner>>,
    task_id: Rc<RefCell<u64>>,
}

impl TokIOBatcher {
    pub fn new(rt: RuntimeType) -> Self {
        Self {
            inner: Rc::new(RefCell::new(TokIOBatcherInner {
                tasks: HashMap::new(),
                rt,
            })),
            task_id: Default::default(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_on_runtime<S: Send + Sync + 'static, F>(
        &self,
        task: F,
        storage_task: Arc<tokio::sync::Mutex<anyhow::Result<S>>>,
        task_finished: Arc<AtomicBool>,
    ) -> TaskJoinType
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let _g = self.inner.borrow_mut().rt.enter();
        tokio::spawn(async move {
            let storage_wrapped = task.await;
            *storage_task.lock().await = storage_wrapped;
            task_finished.store(true, std::sync::atomic::Ordering::SeqCst);
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn spawn_on_runtime<S: Send + Sync + 'static, F>(
        &self,
        task: F,
        storage_task: Arc<tokio::sync::Mutex<anyhow::Result<S>>>,
        task_finished: Arc<AtomicBool>,
    ) -> TaskJoinType
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        self.inner.borrow_mut().rt.spawn(async move {
            let storage_wrapped = task.await;
            *storage_task.lock().await = storage_wrapped;
            task_finished.store(true, std::sync::atomic::Ordering::SeqCst);
        })
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
    ) -> (TokIOBatcherTask<S>, TaskJoinType)
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let id = u64::MAX;

        let storage_task = Arc::new(tokio::sync::Mutex::new(Err(anyhow!("not started yet"))));
        let storage_task_clone = storage_task.clone();

        let task_finished = Arc::new(AtomicBool::new(false));
        let task_finished_clone = task_finished.clone();

        let join_handle = self.spawn_on_runtime(task, storage_task, task_finished_clone);

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
