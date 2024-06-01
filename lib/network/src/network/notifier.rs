use std::{
    sync::{Arc, Weak},
    time::Duration,
};

use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct NetworkEventNotifier {
    pub(crate) rt: Weak<tokio::runtime::Runtime>,
    pub(crate) notify: Arc<Notify>,
}

impl NetworkEventNotifier {
    /// returns false if timeout was exceeded, others always returns true
    pub fn wait_for_event(&self, timeout: Option<Duration>) -> bool {
        NetworkEventNotifierRef {
            notifiers: [&self.notify],
            rt: &self.rt,
        }
        .wait_for_event(timeout)
    }

    pub fn join<'a>(&'a self, other: &'a Self) -> NetworkEventNotifierRef<'a, 2> {
        NetworkEventNotifierRef {
            notifiers: [&self.notify, &other.notify],
            rt: &self.rt,
        }
    }
}

pub struct NetworkEventNotifierRef<'a, const N: usize> {
    rt: &'a Weak<tokio::runtime::Runtime>,
    notifiers: [&'a Arc<Notify>; N],
}

impl<'a, const N: usize> NetworkEventNotifierRef<'a, N> {
    /// returns false if timeout was exceeded, others always returns true
    pub fn wait_for_event(&self, timeout: Option<Duration>) -> bool {
        self.rt.upgrade().unwrap().block_on(async {
            let nty = self.notifiers.get(1).map(|n| (*n).clone());
            let task = async move {
                if let Some(nty) = nty {
                    nty.notified().await;
                }
            };
            match timeout {
                Some(timeout) => {
                    let res = tokio::select! {
                        res = tokio::time::timeout(timeout, self.notifiers[0].notified()), if N > 0 => res,
                        res = tokio::time::timeout(timeout, task), if N > 1 => res
                    };
                    match res {
                        Ok(_) => true,
                        Err(_) => false,
                    }
                }
                None => {
                    tokio::select! {
                        _ = self.notifiers[0].notified(), if N > 0 => {},
                        _ = task, if N > 1 => {}
                    }
                    true
                }
            }
        })
    }
}
