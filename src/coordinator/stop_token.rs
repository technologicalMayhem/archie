use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::time::sleep;
use tokio::time::sleep as tokio_sleep;

pub struct StopToken {
    is_stopped: Arc<AtomicBool>,
    children: Vec<Arc<AtomicBool>>,
}

impl StopToken {
    pub fn new() -> Self {
        Self {
            is_stopped: Arc::new(AtomicBool::new(false)),
            children: Vec::new(),
        }
    }

    pub fn child(&mut self) -> Self {
        let value = self.is_stopped.load(Relaxed);
        let is_stopped = Arc::new(AtomicBool::new(value));
        self.children.push(is_stopped.clone());
        Self {
            is_stopped,
            children: Vec::new(),
        }
    }

    pub fn trigger_stop(&self) {
        self.is_stopped.store(true, Relaxed);
        for child in &self.children {
            child.store(true, Relaxed);
        }
    }

    pub fn stopped(&mut self) -> bool {
        if self.is_stopped.load(Relaxed) {
            for child in &self.children {
                child.store(true, Relaxed);
            }
            true
        } else {
            false
        }
    }

    pub async fn wait(&mut self) {
        while !self.stopped() {
            sleep(Duration::from_millis(10)).await;
        }
    }

    pub async fn sleep(&mut self, duration: Duration) {
        select! {
            () = tokio_sleep(duration) => {},
            () = self.wait() => {},
        }
    }
}

impl Drop for StopToken {
    fn drop(&mut self) {
        for child in &self.children {
            child.store(true, Relaxed);
        }
    }
}
