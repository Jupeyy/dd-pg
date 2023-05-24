use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    thread::JoinHandle,
};

use arrayvec::ArrayString;

pub struct WorkerTask<S> {
    pub queue_id: u64,
    storage: Option<S>,
    storage_task: Arc<Mutex<(Option<S>, Option<ArrayString<4096>>)>>,
    is_finished: Arc<AtomicBool>,
}

impl<S> WorkerTask<S> {
    pub fn get_storage(&mut self) -> Result<S, ArrayString<4096>> {
        // first check if we need to get the storage from the task
        if self.storage.is_none() {
            std::mem::swap(&mut self.storage, &mut self.storage_task.lock().unwrap().0);
            if self.storage.is_none() {
                return Err(self.storage_task.lock().unwrap().1.unwrap());
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

pub struct Worker {
    tasks: Arc<Mutex<VecDeque<(u64, Arc<AtomicBool>, Box<dyn FnOnce() + Send + Sync>)>>>,
    cond: Arc<Condvar>,
    task_id: u64,
    thread: JoinHandle<()>,
    is_finished: Arc<AtomicBool>,
}

impl Worker {
    pub fn new() -> Self {
        let cond = Arc::new(Condvar::new());
        let cond_clone = cond.clone();
        let mutex = Arc::new(Mutex::new(VecDeque::<(
            u64,
            Arc<AtomicBool>,
            Box<dyn FnOnce() + Send + Sync>,
        )>::new()));
        let mutex_clone = mutex.clone();

        let mut guard = mutex.lock().unwrap();
        let is_started = Arc::new(AtomicBool::new(false));
        let is_started_clone = is_started.clone();

        let is_finished = Arc::new(AtomicBool::new(false));
        let is_finished_clone = is_finished.clone();

        let handle = std::thread::spawn(move || {
            let g = mutex_clone.lock().unwrap();
            cond_clone.notify_all();
            is_started_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            drop(g);

            loop {
                let mut g = mutex_clone.lock().unwrap();
                if g.is_empty() {
                    g = cond_clone
                        .wait_while(g, |tasks| {
                            !is_finished_clone.load(std::sync::atomic::Ordering::SeqCst)
                                && tasks.is_empty()
                        })
                        .unwrap();
                };

                for (_, is_finished, task) in g.drain(..) {
                    task();
                    is_finished.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                cond_clone.notify_all();
            }
        });

        guard = cond
            .wait_while(guard, |_| {
                !is_started.load(std::sync::atomic::Ordering::Relaxed)
            })
            .unwrap();
        drop(guard);
        Self {
            tasks: mutex,
            cond: cond,
            task_id: 0,
            thread: handle,
            is_finished: is_finished,
        }
    }

    pub fn spawn<S: Send + Sync + 'static, F>(&mut self, task: F) -> WorkerTask<S>
    where
        F: FnOnce() -> Result<S, ArrayString<4096>> + Send + Sync + 'static,
    {
        let id = self.task_id;
        self.task_id += 1;

        let storage_task = Arc::new(Mutex::new((
            Option::<S>::None,
            Option::<ArrayString<4096>>::None,
        )));
        let storage_task_clone = storage_task.clone();

        let task_finished = Arc::new(AtomicBool::new(false));
        let task_finished_clone = task_finished.clone();

        let mut tasks = self.tasks.lock().unwrap();
        tasks.push_back((
            id,
            task_finished.clone(),
            Box::new(move || {
                let storage_wrapped = task();
                if let Ok(storage) = storage_wrapped {
                    *storage_task.lock().unwrap() = (Some(storage), None);
                } else if let Err(err) = storage_wrapped {
                    *storage_task.lock().unwrap() = (None, Some(err));
                }
                task_finished_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            }),
        ));
        self.cond.notify_all();
        drop(tasks);

        WorkerTask::<S> {
            queue_id: id,
            storage: None,
            storage_task: storage_task_clone,
            is_finished: task_finished,
        }
    }

    pub fn wait_finished<S>(&mut self, task: &mut WorkerTask<S>) {
        if !task.is_finished.load(std::sync::atomic::Ordering::SeqCst) {
            let g = self.tasks.lock().unwrap();
            self.cond.wait_while(g, |_| {
                !task.is_finished.load(std::sync::atomic::Ordering::SeqCst)
            });
        }
    }

    pub fn finish_all(&mut self) {
        let g = self.tasks.lock().unwrap();
        self.cond.wait_while(g, |tasks| !tasks.is_empty());
    }
}
