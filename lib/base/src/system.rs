use std::{sync::Arc, time::Duration};

use arrayvec::ArrayString;

#[derive(Clone)]
pub struct LogItem {
    msg: ArrayString<4096>,
}

impl LogItem {
    pub fn new() -> Self {
        Self {
            msg: Default::default(),
        }
    }
}

pub struct LogItemConcat<'a> {
    log_item: &'a mut LogItem,
}

pub trait SystemTimeInterface {
    fn time_get_nanoseconds(&self) -> Duration;
}

pub trait SystemLogInterface {
    /**
     * Logs useful informations
     */
    fn log(&mut self, sys: &str) -> LogItemConcat;
    /**
     * Logs information is rather verbose or only useful for developers
     */
    fn log_debug(&mut self, sys: &str) -> LogItemConcat;
}

impl<'a> LogItemConcat<'a> {
    pub fn msg(&mut self, msg_str: &str) -> &mut Self {
        self.log_item.msg.push_str(msg_str);
        self
    }

    pub fn msg_var<T: ToString>(&mut self, val: &T) -> &mut Self {
        self.log_item.msg.push_str(val.to_string().as_str());
        self
    }
}

impl<'a> Drop for LogItemConcat<'a> {
    fn drop(&mut self) {
        println!("{}", self.log_item.msg.as_str());
    }
}

#[derive(Clone)]
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
pub struct SystemLog {
    logs: Vec<LogItem>,
    logs_debug: Vec<LogItem>,
}

impl SystemLog {
    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            logs_debug: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct System {
    pub time: Arc<SystemTime>,
    pub log: SystemLog,
}

impl System {
    pub fn new() -> System {
        System {
            time: Arc::new(SystemTime::new()),
            log: SystemLog::new(),
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

impl SystemLogInterface for SystemLog {
    fn log(&mut self, _sys: &str) -> LogItemConcat {
        self.logs.push(LogItem::new());
        let r = self.logs.last_mut().unwrap();
        LogItemConcat { log_item: r }
    }

    fn log_debug(&mut self, _sys: &str) -> LogItemConcat {
        self.logs_debug.push(LogItem::new());
        let r = self.logs_debug.last_mut().unwrap();
        LogItemConcat { log_item: r }
    }
}

impl SystemLogInterface for System {
    fn log(&mut self, sys: &str) -> LogItemConcat {
        self.log.log(sys)
    }

    fn log_debug(&mut self, sys: &str) -> LogItemConcat {
        self.log.log_debug(sys)
    }
}

pub trait SystemInterface
where
    Self: SystemTimeInterface + SystemLogInterface,
{
}

impl SystemInterface for System {}
