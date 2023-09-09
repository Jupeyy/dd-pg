use std::{fmt::Debug, sync::Arc, time::Duration};

use base_log::log::SystemLog;

pub trait SystemTimeInterface {
    fn time_get_nanoseconds(&self) -> Duration;
}

#[derive(Debug, Clone)]
pub struct SystemTime {
    sys_start_time: std::time::Instant,
}

impl SystemTime {
    pub fn new() -> Self {
        Self {
            sys_start_time: std::time::Instant::now(),
        }
    }
}

#[derive(Clone)]
pub struct System {
    pub time: Arc<SystemTime>,
    pub log: Arc<SystemLog>,
}

impl System {
    pub fn new() -> System {
        System {
            time: Arc::new(SystemTime::new()),
            log: Arc::new(SystemLog::new()),
        }
    }
}

impl SystemTimeInterface for SystemTime {
    fn time_get_nanoseconds(&self) -> Duration {
        let diff_to_start = std::time::Instant::now().duration_since(self.sys_start_time);
        return diff_to_start;
    }
}

impl SystemTimeInterface for System {
    fn time_get_nanoseconds(&self) -> Duration {
        self.time.time_get_nanoseconds()
    }
}

pub trait SystemInterface
where
    Self: SystemTimeInterface,
{
}

impl SystemInterface for System {}
