use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

pub struct OneTimeSignal {
    ready: AtomicBool,
    notify: Notify,
}

impl OneTimeSignal {
    pub fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    pub fn set(&self) {
        if !self.ready.swap(true, Ordering::SeqCst) {
            self.notify.notify_waiters();
        }
    }

    pub async fn wait(&self) {
        if self.ready.load(Ordering::SeqCst) {
            return;
        }

        loop {
            self.notify.notified().await;
            if self.ready.load(Ordering::SeqCst) {
                break;
            }
        }
    }
}
