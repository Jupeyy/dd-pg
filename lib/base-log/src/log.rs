use std::{
    cell::RefCell,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use arrayvec::ArrayString;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warning,
    Error,
}

#[cfg(target_arch = "wasm32")]
extern "Rust" {
    fn sys_print(str: &str);
}

pub trait SystemLogInterface {
    /**
     * Logs useful informations grouped by a log level
     */
    fn log(&self, log_level: LogLevel) -> LogItemConcat;
}

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
            #[cfg(not(target_arch = "wasm32"))]
            println!("{}", tmp.msg.as_str());
            #[cfg(target_arch = "wasm32")]
            unsafe {
                sys_print(&format!("{}", tmp.msg.as_str()))
            };
        }
        self.log_items.borrow_mut().push(tmp);
    }
}

#[derive(Debug)]
pub struct SystemLogGroup {
    _name: String, // TODO:
    local_logs: RefCell<Vec<LogItem>>,
    _global_logs: Arc<Mutex<String>>, // TODO:
}

impl SystemLogGroup {
    fn new(name: String, global_logs: Arc<Mutex<String>>) -> Self {
        Self {
            _name: name,
            local_logs: RefCell::new(Vec::with_capacity(512)),
            _global_logs: global_logs,
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
    pub fn new() -> Self {
        Self {
            global_logs: Default::default(),
        }
    }

    pub fn logger(&self, sys: &str) -> SystemLogGroup {
        SystemLogGroup::new(sys.to_string(), self.global_logs.clone())
    }
}