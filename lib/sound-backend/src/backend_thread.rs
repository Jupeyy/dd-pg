use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
};

use anyhow::anyhow;
use config::config::ConfigSound;
use hiarc::Hiarc;
use parking_lot::{Condvar, Mutex};

use crate::backend::{
    kira::kira::{SoundBackendKira, SoundBackendMtKira},
    null::null::SoundBackendNull,
};
use sound::{
    backend_types::{SoundBackendDriverInterface, SoundBackendMtDriverInterface},
    commands::SoundCommand,
};

#[derive(Debug, Hiarc)]
enum SoundBackendType {
    Kira(Box<SoundBackendKira>),
    Null(SoundBackendNull),
}

impl SoundBackendType {
    pub fn as_mut(&mut self) -> &mut dyn SoundBackendDriverInterface {
        match self {
            SoundBackendType::Kira(backend) => backend.as_mut(),
            SoundBackendType::Null(backend) => backend,
        }
    }
}

#[derive(Debug, Hiarc)]
pub(crate) enum SoundBackendMtType {
    Kira(SoundBackendMtKira),
    Null(SoundBackendNull),
}

impl SoundBackendMtType {
    pub(crate) fn as_ref(&self) -> &dyn SoundBackendMtDriverInterface {
        match self {
            SoundBackendMtType::Kira(backend) => backend,
            SoundBackendMtType::Null(backend) => backend,
        }
    }
}

#[derive(Debug, Hiarc)]
pub enum BackendThreadBackendEvent {
    Init { backend: String },
    RunCmds { cmds: Vec<SoundCommand> },
    Stop,
}

#[derive(Debug, Hiarc)]
enum BackendThreadFrontendEvent {
    InitRes {
        backend_mt: SoundBackendMtType,
    },
    /// send every time commands were handled. Useful to make sure the backend doesn't get overloaded
    Sync,
}

#[derive(Debug, Hiarc, Default)]
struct BackendThreadEvents {
    backend_events: VecDeque<BackendThreadBackendEvent>,
    frontend_events: VecDeque<BackendThreadFrontendEvent>,
}

#[derive(Debug, Hiarc)]
pub struct SoundBackendThread {
    thread: Option<JoinHandle<anyhow::Result<()>>>,

    is_finished: Arc<AtomicBool>,
    nty: Arc<Condvar>,
    events: Arc<Mutex<BackendThreadEvents>>,
}

impl SoundBackendThread {
    pub(crate) fn new(config: &ConfigSound) -> anyhow::Result<(Self, SoundBackendMtType)> {
        let is_finished: Arc<AtomicBool> = Default::default();
        let nty: Arc<Condvar> = Default::default();
        let events: Arc<Mutex<BackendThreadEvents>> = Default::default();

        events
            .lock()
            .backend_events
            .push_back(BackendThreadBackendEvent::Init {
                backend: config.backend.clone(),
            });

        let is_finished_clone = is_finished.clone();
        let nty_clone = nty.clone();
        let events_clone = events.clone();

        let thread = std::thread::Builder::new()
            .name("sound-backend-thread".into())
            .spawn(move || {
                match SoundBackendThread::run(nty_clone, events_clone, is_finished_clone) {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        println!("graphics backend thread died: {err} {}", err.backtrace());
                        Err(err)
                    }
                }
            })
            .ok();

        let mut g = events.lock();
        nty.wait_while(&mut g, |g| g.frontend_events.is_empty());
        let Some(BackendThreadFrontendEvent::InitRes { backend_mt }) =
            g.frontend_events.pop_front()
        else {
            return Err(anyhow!("missing init event response"));
        };
        drop(g);

        Ok((
            Self {
                thread,
                events,
                is_finished,
                nty,
            },
            backend_mt,
        ))
    }

    pub fn run_cmds(&self, cmds: Vec<SoundCommand>) -> anyhow::Result<()> {
        let mut g = self.events.lock();
        self.nty
            .wait_while(&mut g, |g| g.frontend_events.is_empty());
        let Some(BackendThreadFrontendEvent::Sync) = g.frontend_events.pop_front() else {
            return Err(anyhow!("Sync event not found"));
        };

        g.backend_events
            .push_back(BackendThreadBackendEvent::RunCmds { cmds });
        self.nty.notify_all();

        Ok(())
    }

    fn run(
        nty: Arc<Condvar>,
        events: Arc<Mutex<BackendThreadEvents>>,

        is_finished: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        let mut g = events.lock();

        // handle loading
        let load_ev = g.backend_events.pop_front();
        let Some(BackendThreadBackendEvent::Init { backend }) = load_ev else {
            return Err(anyhow!("first event is always the load event"));
        };
        let mut backend = match backend.as_str() {
            "kira" => match SoundBackendKira::new() {
                Ok(backend) => SoundBackendType::Kira(backend),
                _ => SoundBackendType::Null(SoundBackendNull {}),
            },
            _ => SoundBackendType::Null(SoundBackendNull {}),
        };
        g.frontend_events
            .push_back(BackendThreadFrontendEvent::InitRes {
                backend_mt: match &backend {
                    SoundBackendType::Kira(backend) => {
                        SoundBackendMtType::Kira(backend.get_backend_mt())
                    }
                    SoundBackendType::Null(_) => SoundBackendMtType::Null(SoundBackendNull {}),
                },
            });
        nty.notify_all();
        g.frontend_events
            .push_back(BackendThreadFrontendEvent::Sync);
        nty.notify_all();

        'outer: while !is_finished.load(std::sync::atomic::Ordering::SeqCst) {
            nty.wait_while(&mut g, |g| g.backend_events.is_empty());

            let evs = &mut *g;
            for event in evs.backend_events.drain(..) {
                match event {
                    BackendThreadBackendEvent::Init { .. } => {
                        panic!("backend was already initialized")
                    }
                    BackendThreadBackendEvent::RunCmds { cmds } => {
                        backend.as_mut().run_cmds(cmds)?;
                        evs.frontend_events
                            .push_back(BackendThreadFrontendEvent::Sync);
                    }
                    BackendThreadBackendEvent::Stop => break 'outer,
                }
            }
            nty.notify_all();
        }

        g.backend_events.clear();
        nty.notify_all();

        Ok(())
    }
}

impl Drop for SoundBackendThread {
    fn drop(&mut self) {
        let mut g = self.events.lock();
        self.is_finished
            .store(true, std::sync::atomic::Ordering::SeqCst);
        g.backend_events.push_back(BackendThreadBackendEvent::Stop);
        self.nty.notify_all();
        self.nty
            .wait_while(&mut g, |g| !g.backend_events.is_empty());

        drop(g);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
