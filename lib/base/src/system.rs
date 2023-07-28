use std::{
    cell::RefCell,
    fmt::Debug,
    sync::{Arc, Mutex},
    time::Duration,
};

use arrayvec::ArrayString;

#[derive(Debug, Clone)]
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
    log_item: LogItem,
    log_level: LogLevel,
    log_items: &'a RefCell<Vec<LogItem>>,
}

pub trait SystemTimeInterface {
    fn time_get_nanoseconds(&self) -> Duration;
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warning,
    Error,
}

pub trait SystemLogInterface {
    /**
     * Logs useful informations grouped by a log level
     */
    fn log(&self, log_level: LogLevel) -> LogItemConcat;
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

    pub fn msg_dbg<T: Debug>(&mut self, val: T) -> &mut Self {
        self.log_item.msg.push_str(&format!("{:?}", val));
        self
    }
}

impl<'a> Drop for LogItemConcat<'a> {
    fn drop(&mut self) {
        let mut tmp = LogItem::new();
        std::mem::swap(&mut self.log_item, &mut tmp);
        if self.log_level > LogLevel::Verbose {
            println!("{}", tmp.msg.as_str());
        }
        self.log_items.borrow_mut().push(tmp);
    }
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

#[derive(Debug)]
pub struct SystemLogGroup {
    name: String,
    local_logs: RefCell<Vec<LogItem>>,
    global_logs: Arc<Mutex<String>>,
}

impl SystemLogGroup {
    fn new(global_logs: Arc<Mutex<String>>) -> Self {
        Self {
            name: Default::default(),
            local_logs: RefCell::new(Vec::with_capacity(512)),
            global_logs,
        }
    }
}

impl SystemLogInterface for SystemLogGroup {
    fn log(&self, log_level: LogLevel) -> LogItemConcat {
        LogItemConcat {
            log_item: LogItem::new(),
            log_level,
            log_items: &self.local_logs,
        }
    }
}

#[derive(Debug)]
pub struct SystemLog {
    global_logs: Arc<Mutex<String>>,
}

impl SystemLog {
    fn new() -> Self {
        Self {
            global_logs: Default::default(),
        }
    }

    pub fn logger(&self, sys: &str) -> SystemLogGroup {
        SystemLogGroup {
            name: sys.to_string(),
            local_logs: Default::default(),
            global_logs: self.global_logs.clone(),
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
