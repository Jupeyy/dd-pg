use std::{
    collections::HashMap,
    future::Future,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::anyhow;
use base::system::{LogLevel, SystemLog, SystemLogGroup, SystemLogInterface};
use tokio::task::JoinHandle;

pub struct TokIOBatcherTask<S> {
    pub queue_id: u64,
    storage: Option<S>,
    storage_task: Arc<tokio::sync::Mutex<(Option<S>, Option<anyhow::Error>)>>,
    is_finished: Arc<AtomicBool>,
}

impl<S> TokIOBatcherTask<S> {
    pub fn get_storage(&mut self) -> anyhow::Result<S> {
        // first check if we need to get the storage from the task
        if self.storage.is_none() {
            std::mem::swap(&mut self.storage, &mut self.storage_task.blocking_lock().0);
            if self.storage.is_none() {
                return Err(anyhow!(self
                    .storage_task
                    .blocking_lock()
                    .1
                    .as_mut()
                    .unwrap()
                    .to_string()));
            }
        }
        // then move out the storage
        let mut s = None;
        std::mem::swap(&mut s, &mut self.storage);
        Ok(s.unwrap())
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub struct TokIOBatcher {
    tasks: HashMap<u64, JoinHandle<()>>,
    #[cfg(debug_assertions)]
    tasks_bt: HashMap<u64, String>,
    task_id: u64,
    rt: tokio::runtime::Runtime,
    logger: SystemLogGroup,
}

impl TokIOBatcher {
    pub fn new(rt: tokio::runtime::Runtime, log: &SystemLog) -> Self {
        Self {
            tasks: HashMap::new(),
            #[cfg(debug_assertions)]
            tasks_bt: Default::default(),
            task_id: 0,
            rt: rt,
            logger: log.logger("io_batcher"),
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
        &mut self,
        task: F,
    ) -> (TokIOBatcherTask<S>, JoinHandle<()>)
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let id = u64::MAX;

        let storage_task = Arc::new(tokio::sync::Mutex::new((
            Option::<S>::None,
            Option::<anyhow::Error>::None,
        )));
        let storage_task_clone = storage_task.clone();

        let task_finished = Arc::new(AtomicBool::new(false));
        let task_finished_clone = task_finished.clone();

        let _g = self.rt.enter();
        let join_handle = tokio::spawn(async move {
            let storage_wrapped = task.await;
            if let Ok(storage) = storage_wrapped {
                *storage_task.lock().await = (Some(storage), None);
            } else if let Err(err) = storage_wrapped {
                *storage_task.lock().await = (None, Some(err));
            }
            task_finished_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        (
            TokIOBatcherTask::<S> {
                queue_id: id,
                storage: None,
                storage_task: storage_task_clone,
                is_finished: task_finished,
            },
            join_handle,
        )
    }

    pub fn spawn<S: Send + Sync + 'static, F>(&mut self, task: F) -> TokIOBatcherTask<S>
    where
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        let mut task = self.spawn_without_queue(task);
        task.0.queue_id = self.task_id;
        self.task_id += 1;

        self.tasks.insert(task.0.queue_id, task.1);
        #[cfg(debug_assertions)]
        self.tasks_bt.insert(
            task.0.queue_id,
            std::backtrace::Backtrace::force_capture().to_string(),
        );

        task.0
    }

    pub fn wait_finished_and_drop<S>(&mut self, task: &mut TokIOBatcherTask<S>) {
        #[cfg(debug_assertions)]
        self.tasks_bt.remove(&task.queue_id).unwrap();
        let task_join = self.tasks.remove(&task.queue_id).unwrap();
        tokio::task::block_in_place(|| {
            self.rt.block_on(async move { task_join.await }).unwrap();
        });
    }
}

impl Drop for TokIOBatcher {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if self.tasks.len() > 0 {
            let mut logger = self.logger.log(LogLevel::Info);
            logger.msg("The following tasks were not cleared:");
            self.tasks_bt.values().for_each(|bt| {
                logger.msg(&bt);
            });
        }
        assert!(self.tasks.len() == 0, "tasks were not cleared correctly");
    }
}
