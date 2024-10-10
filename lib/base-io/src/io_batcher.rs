use std::{
    cell::{Cell, RefCell, RefMut},
    collections::HashMap,
    future::Future,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::anyhow;
use hiarc::Hiarc;

#[cfg(not(target_arch = "wasm32"))]
pub type RuntimeType = tokio::runtime::Runtime;
#[cfg(target_arch = "wasm32")]
pub type RuntimeType = async_executor::LocalExecutor<'static>;

#[cfg(not(target_arch = "wasm32"))]
pub type TaskJoinType = tokio::task::JoinHandle<()>;
#[cfg(target_arch = "wasm32")]
pub type TaskJoinType = async_task::Task<()>;

#[derive(Debug, Hiarc)]
enum TaskState {
    WaitAndDrop,
    CancelAndDrop,
    None,
}

#[derive(Debug, Hiarc)]
pub struct IoBatcherTask<S> {
    pub queue_id: u64,
    storage_task: Arc<tokio::sync::Mutex<anyhow::Result<S>>>,
    is_finished: Arc<AtomicBool>,

    // private
    io_batcher: Rc<RefCell<IoBatcherInner>>,
    task_state: TaskState,
}

impl<S> IoBatcherTask<S> {
    fn drop_task(queue_id: u64, inner: &mut RefMut<IoBatcherInner>) -> TaskJoinType {
        inner
            .tasks
            .remove(&queue_id)
            .ok_or_else(|| anyhow!("Could not find queue id {} in {:?}", queue_id, inner.tasks))
            .unwrap()
    }

    fn wait_finished_and_drop(&mut self) {
        let mut inner = self.io_batcher.borrow_mut();
        let task_join = Self::drop_task(self.queue_id, &mut inner);
        #[cfg(not(target_arch = "wasm32"))]
        inner.rt.block_on(task_join).unwrap();
        #[cfg(target_arch = "wasm32")]
        futures_lite::future::block_on(inner.rt.run(task_join));
        self.task_state = TaskState::None;
    }

    pub fn blocking_wait_finished(&mut self) {
        if let TaskState::WaitAndDrop | TaskState::CancelAndDrop = self.task_state {
            self.wait_finished_and_drop();
        }
    }

    pub fn get_storage(mut self) -> anyhow::Result<S> {
        self.blocking_wait_finished();
        let mut storage_res = Err(anyhow!("not started yet"));
        std::mem::swap(&mut *self.storage_task.blocking_lock(), &mut storage_res);
        storage_res
    }

    #[cfg(target_arch = "wasm32")]
    fn try_run(&self) {
        let inner = self.io_batcher.borrow_mut();
        inner.rt.try_tick();
    }

    pub fn is_finished(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        self.try_run();
        self.is_finished.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// this function makes a task that was spawned using the task queue ([`IoBatcher::spawn`])
    /// cancelable and automatically abort on drop
    pub fn cancelable(mut self) -> Self {
        if let TaskState::WaitAndDrop | TaskState::CancelAndDrop = self.task_state {
            self.task_state = TaskState::CancelAndDrop;
        } else {
            panic!("the cancelable call has no effect on this task, because it was not part of the task queue. Use the join handle directly.");
        }
        self
    }

    /// Shares the same logic as [`Self::cancelable`].
    /// Just there for expediency.
    pub fn abortable(self) -> Self {
        self.cancelable()
    }
}

impl<S> Drop for IoBatcherTask<S> {
    fn drop(&mut self) {
        match self.task_state {
            TaskState::WaitAndDrop => {
                self.wait_finished_and_drop();
            }
            TaskState::CancelAndDrop => {
                let mut inner = self.io_batcher.borrow_mut();
                let task = Self::drop_task(self.queue_id, &mut inner);
                #[cfg(not(target_arch = "wasm32"))]
                task.abort();
                #[cfg(target_arch = "wasm32")]
                let _ = task.cancel();
            }
            TaskState::None => {
                // nothing to do
            }
        }
    }
}

#[derive(Debug, Hiarc)]
struct IoBatcherInner {
    #[hiarc_skip_unsafe]
    tasks: HashMap<u64, TaskJoinType>,
    #[hiarc_skip_unsafe]
    lifetimeless_tasks: HashMap<u64, TaskJoinType>,
    #[hiarc_skip_unsafe]
    rt: RuntimeType,
}

impl Drop for IoBatcherInner {
    fn drop(&mut self) {
        let mut tasks = Default::default();
        std::mem::swap(&mut tasks, &mut self.tasks);
        for (_, task) in tasks.drain() {
            #[cfg(not(target_arch = "wasm32"))]
            self.rt.block_on(task).unwrap();
            #[cfg(target_arch = "wasm32")]
            let _ = self.rt.run(task);
        }
        let mut lifetimeless_tasks = Default::default();
        std::mem::swap(&mut lifetimeless_tasks, &mut self.lifetimeless_tasks);
        for (_, task) in lifetimeless_tasks.drain() {
            #[cfg(not(target_arch = "wasm32"))]
            self.rt.block_on(task).unwrap();
            #[cfg(target_arch = "wasm32")]
            let _ = self.rt.run(task);
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct IoBatcher {
    inner: Rc<RefCell<IoBatcherInner>>,
    task_id: Rc<Cell<u64>>,
}

impl IoBatcher {
    pub fn new(rt: RuntimeType) -> Self {
        Self {
            inner: Rc::new(RefCell::new(IoBatcherInner {
                tasks: HashMap::new(),
                lifetimeless_tasks: HashMap::new(),
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

    #[must_use]
    fn spawn_impl<S: Send + Sync + 'static, F>(&self, task: F) -> (IoBatcherTask<S>, TaskJoinType)
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
            IoBatcherTask::<S> {
                queue_id: id,
                storage_task: storage_task_clone,
                is_finished: task_finished,
                io_batcher: self.inner.clone(),
                task_state: TaskState::None,
            },
            join_handle,
        )
    }

    /// This function spawns a task that has no result type and will ran async (without any lifetime).
    /// There is no guarantee about the order of execution or when the task finishes.
    /// The only guarantee is, that the spawned task will be waited for to be finished at the destruction of the io-batcher instance.
    /// Generally this function is only recommended for destructors (Drop) to save some runtime cost (by not waiting for the task).
    pub fn spawn_without_lifetime<F>(&self, task: F)
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let (mut task, join_handle) = self.spawn_impl(task);
        task.queue_id = self.task_id.replace(self.task_id.get() + 1);

        let mut inner = self.inner.borrow_mut();

        inner
            .lifetimeless_tasks
            .retain(|_, task| !task.is_finished());

        inner.lifetimeless_tasks.insert(task.queue_id, join_handle);
    }

    #[must_use]
    pub fn spawn<S: Send + Sync + 'static, F>(&self, task: F) -> IoBatcherTask<S>
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let (mut task, join_handle) = self.spawn_impl(task);
        task.queue_id = self.task_id.replace(self.task_id.get() + 1);

        self.inner
            .borrow_mut()
            .tasks
            .insert(task.queue_id, join_handle);

        task.task_state = TaskState::WaitAndDrop;
        task
    }

    /// Creates a new task that takes the result of the given task as parameter
    /// and creates a new async task out of it.
    #[must_use]
    pub fn then<P: Send + Sync + 'static, S: Send + Sync + 'static, F, N>(
        &self,
        mut task: IoBatcherTask<P>,
        f: N,
    ) -> IoBatcherTask<S>
    where
        N: FnOnce(P) -> F + Send + 'static,
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let task_join =
            IoBatcherTask::<P>::drop_task(task.queue_id, &mut task.io_batcher.borrow_mut());
        task.task_state = TaskState::None;
        let storage_task = task.storage_task.clone();

        self.spawn(async move {
            #[cfg(not(target_arch = "wasm32"))]
            task_join.await?;
            #[cfg(target_arch = "wasm32")]
            task_join.await;

            let mut storage_res = Err(anyhow!("not started yet"));
            std::mem::swap(&mut *storage_task.lock().await, &mut storage_res);
            let storage_res = storage_res?;

            f(storage_res).await
        })
    }
}
