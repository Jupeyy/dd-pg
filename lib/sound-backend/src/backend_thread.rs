use std::{
    sync::mpsc::{Receiver, Sender},
    thread::JoinHandle,
};

use anyhow::anyhow;
use config::config::ConfigSound;
use hiarc::Hiarc;

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
}

#[derive(Debug, Hiarc)]
enum BackendThreadFrontendEvent {
    InitRes {
        backend_mt: SoundBackendMtType,
    },
    /// send every time commands were handled. Useful to make sure the backend doesn't get overloaded
    Sync,
}

#[derive(Debug, Hiarc)]
struct JoinThread(Option<JoinHandle<anyhow::Result<()>>>);

impl Drop for JoinThread {
    fn drop(&mut self) {
        if let Some(thread) = self.0.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct SoundBackendThread {
    events: Sender<BackendThreadBackendEvent>,
    recv_events: Receiver<BackendThreadFrontendEvent>,

    // custom drop, must stay last element
    _thread: JoinThread,
}

impl SoundBackendThread {
    pub(crate) fn new(config: &ConfigSound) -> anyhow::Result<(Self, SoundBackendMtType)> {
        let (events, recv) = std::sync::mpsc::channel();
        let (sender, frontent_events) = std::sync::mpsc::channel();

        events.send(BackendThreadBackendEvent::Init {
            backend: config.backend.clone(),
        })?;

        let thread = std::thread::Builder::new()
            .name("sound-backend-thread".into())
            .spawn(move || match SoundBackendThread::run(recv, sender) {
                Ok(_) => Ok(()),
                Err(err) => {
                    println!("sounds backend thread died: {err} {}", err.backtrace());
                    Err(err)
                }
            })?;

        let BackendThreadFrontendEvent::InitRes { backend_mt } = frontent_events.recv()? else {
            return Err(anyhow!("missing init event response"));
        };

        Ok((
            Self {
                events,
                recv_events: frontent_events,
                _thread: JoinThread(Some(thread)),
            },
            backend_mt,
        ))
    }

    pub fn run_cmds(&self, cmds: Vec<SoundCommand>) -> anyhow::Result<()> {
        let BackendThreadFrontendEvent::Sync = self.recv_events.recv()? else {
            return Err(anyhow!("Sync event not found"));
        };

        self.events
            .send(BackendThreadBackendEvent::RunCmds { cmds })?;

        Ok(())
    }

    fn run(
        events: Receiver<BackendThreadBackendEvent>,
        sender: Sender<BackendThreadFrontendEvent>,
    ) -> anyhow::Result<()> {
        // handle loading
        let load_ev = events.recv()?;
        let BackendThreadBackendEvent::Init { backend } = load_ev else {
            return Err(anyhow!("first event is always the load event"));
        };
        let mut backend = match backend.as_str() {
            "kira" => match SoundBackendKira::new() {
                Ok(backend) => SoundBackendType::Kira(backend),
                _ => SoundBackendType::Null(SoundBackendNull {}),
            },
            _ => SoundBackendType::Null(SoundBackendNull {}),
        };
        sender.send(BackendThreadFrontendEvent::InitRes {
            backend_mt: match &backend {
                SoundBackendType::Kira(backend) => {
                    SoundBackendMtType::Kira(backend.get_backend_mt())
                }
                SoundBackendType::Null(_) => SoundBackendMtType::Null(SoundBackendNull {}),
            },
        })?;
        sender.send(BackendThreadFrontendEvent::Sync)?;

        while let Ok(event) = events.recv() {
            match event {
                BackendThreadBackendEvent::Init { .. } => {
                    panic!("backend was already initialized")
                }
                BackendThreadBackendEvent::RunCmds { cmds } => {
                    backend.as_mut().run_cmds(cmds)?;
                    sender.send(BackendThreadFrontendEvent::Sync)?;
                }
            }
        }

        Ok(())
    }
}
