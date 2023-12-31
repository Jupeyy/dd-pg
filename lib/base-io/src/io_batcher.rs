use std::{
    cell::{Cell, RefCell, RefMut},
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
enum TaskState {
    WaitAndDrop,
    CancelAndDrop,
    None,
}

#[derive(Debug)]
pub struct IOBatcherTask<S> {
    pub queue_id: u64,
    storage_task: Arc<tokio::sync::Mutex<anyhow::Result<S>>>,
    is_finished: Arc<AtomicBool>,

    // private
    io_batcher: Rc<RefCell<IOBatcherInner>>,
    task_state: TaskState,
}

impl<S> IOBatcherTask<S> {
    fn drop_task(queue_id: u64, inner: &mut RefMut<IOBatcherInner>) -> TaskJoinType {
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

    pub fn get_storage(mut self) -> anyhow::Result<S> {
        if let TaskState::WaitAndDrop | TaskState::CancelAndDrop = self.task_state {
            self.wait_finished_and_drop();
        }
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

    /// this function makes a task that was spawned using the task queue (`spawn`)
    /// cancelable and automatically abort on drop
    pub fn cancelable(mut self) -> Self {
        if let TaskState::WaitAndDrop | TaskState::CancelAndDrop = self.task_state {
            self.task_state = TaskState::CancelAndDrop;
        } else {
            panic!("the cancelable call has no effect on this task, because it was not part of the task queue. Use the join handle directly.");
        }
        self
    }
}

impl<S> Drop for IOBatcherTask<S> {
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
                task.cancel();
            }
            TaskState::None => {
                // nothing to do
            }
        }
    }
}

#[derive(Debug)]
struct IOBatcherInner {
    tasks: HashMap<u64, TaskJoinType>,
    rt: RuntimeType,
}

impl Drop for IOBatcherInner {
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
pub struct IOBatcher {
    inner: Rc<RefCell<IOBatcherInner>>,
    task_id: Rc<Cell<u64>>,
}

impl IOBatcher {
    pub fn new(rt: RuntimeType) -> Self {
        Self {
            inner: Rc::new(RefCell::new(IOBatcherInner {
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

    /// If you are sure you want to create a task without pushing it to the internal queue,
    /// you can do so:
    /// Pros:
    ///  - Control over the join handle of this task
    ///  - If the runtime supports proper task spawning, this task can "leak" in a sense that it will continue running
    /// Cons:
    ///  - You have to manually make sure the task is finished before calling `get_storage` on it
    /// Generally it's not recommended to use this function, except you need exactly these guarantees
    /// or want to have the smallest performance gains possible
    #[must_use]
    pub fn spawn_without_queue<S: Send + Sync + 'static, F>(
        &self,
        task: F,
    ) -> (IOBatcherTask<S>, TaskJoinType)
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
            IOBatcherTask::<S> {
                queue_id: id,
                storage_task: storage_task_clone,
                is_finished: task_finished,
                io_batcher: self.inner.clone(),
                task_state: TaskState::None,
            },
            join_handle,
        )
    }

    #[must_use]
    pub fn spawn<S: Send + Sync + 'static, F>(&self, task: F) -> IOBatcherTask<S>
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let (mut task, join_handle) = self.spawn_without_queue(task);
        task.queue_id = self.task_id.replace(self.task_id.get() + 1);

        self.inner
            .borrow_mut()
            .tasks
            .insert(task.queue_id, join_handle);

        task.task_state = TaskState::WaitAndDrop;
        task
    }
}
