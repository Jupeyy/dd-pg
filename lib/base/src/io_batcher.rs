use std::{
    collections::VecDeque,
    future::Future,
    sync::{atomic::AtomicBool, Arc},
};

use arrayvec::ArrayString;
use tokio::task::JoinHandle;

pub struct IOBatcherTask<S> {
    pub queue_id: u64,
    storage: Option<S>,
    storage_task: Arc<tokio::sync::Mutex<(Option<S>, Option<ArrayString<4096>>)>>,
    is_finished: Arc<AtomicBool>,
}

impl<S> IOBatcherTask<S> {
    pub fn get_storage(&mut self) -> Result<S, ArrayString<4096>> {
        // first check if we need to get the storage from the task
        if self.storage.is_none() {
            std::mem::swap(&mut self.storage, &mut self.storage_task.blocking_lock().0);
            if self.storage.is_none() {
                return Err(self.storage_task.blocking_lock().1.unwrap());
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

pub struct IOBatcher {
    tasks: VecDeque<(u64, Arc<AtomicBool>, JoinHandle<()>)>,
    task_id: u64,
    rt: tokio::runtime::Runtime,
}

impl IOBatcher {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        Self {
            tasks: VecDeque::new(),
            task_id: 0,
            rt: rt,
        }
    }

    pub fn spawn<S: Send + Sync + 'static, F>(&mut self, task: F) -> IOBatcherTask<S>
    where
        F: Future<Output = Result<S, ArrayString<4096>>> + Send + Sync + 'static,
    {
        let id = self.task_id;
        self.task_id += 1;

        let storage_task = Arc::new(tokio::sync::Mutex::new((
            Option::<S>::None,
            Option::<ArrayString<4096>>::None,
        )));
        let storage_task_clone = storage_task.clone();

        let task_finished = Arc::new(AtomicBool::new(false));
        let task_finished_clone = task_finished.clone();

        let _g = self.rt.enter();
        self.tasks.push_back((
            id,
            task_finished.clone(),
            tokio::spawn(async move {
                let storage_wrapped = task.await;
                if let Ok(storage) = storage_wrapped {
                    *storage_task.lock().await = (Some(storage), None);
                } else if let Err(err) = storage_wrapped {
                    *storage_task.lock().await = (None, Some(err));
                }
                task_finished_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            }),
        ));

        IOBatcherTask::<S> {
            queue_id: id,
            storage: None,
            storage_task: storage_task_clone,
            is_finished: task_finished,
        }
    }

    pub fn wait_finished<S>(&mut self, task: &mut IOBatcherTask<S>) {
        if !task.is_finished.load(std::sync::atomic::Ordering::SeqCst) {
            while !self.tasks.is_empty() {
                let is_task = self.tasks.front().unwrap().0 == task.queue_id;
                let t = self.tasks.pop_front();
                let _g = self.rt.enter();
                tokio::task::block_in_place(|| {
                    self.rt.block_on(async move { t.unwrap().2.await }).unwrap();
                });

                if is_task {
                    break;
                }
            }
        }
    }

    pub fn finish_all(&mut self) {
        while !self.tasks.is_empty() {
            let t = self.tasks.pop_front();
            let _g = self.rt.enter();
            tokio::task::block_in_place(|| {
                self.rt.block_on(async move { t.unwrap().2.await }).unwrap();
            });
        }
    }
}
