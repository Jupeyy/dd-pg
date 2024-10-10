#![allow(clippy::new_without_default)]

use std::{fmt::Debug, sync::Arc, time::Duration};

use hiarc::Hiarc;

pub trait SystemTimeInterface {
    fn time_get_nanoseconds(&self) -> Duration;
}

#[derive(Debug, Hiarc, Clone)]
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

#[derive(Debug, Hiarc, Clone)]
pub struct System {
    pub time: Arc<SystemTime>,
}

impl System {
    pub fn new() -> Self {
        Self {
            time: Arc::new(SystemTime::new()),
        }
    }
}

impl SystemTimeInterface for SystemTime {
    fn time_get_nanoseconds(&self) -> Duration {
        self.sys_start_time.elapsed()
    }
}

impl SystemTimeInterface for System {
    fn time_get_nanoseconds(&self) -> Duration {
        self.time.time_get_nanoseconds()
    }
}

pub trait SystemInterface: Debug
where
    Self: SystemTimeInterface,
{
}

impl SystemInterface for System {}
