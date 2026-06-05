#![allow(dead_code)]

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use std::time::Duration;

pub struct Debouncer {
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    delay: Duration,
}

impl Debouncer {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
            delay: Duration::from_millis(delay_ms),
        }
    }

    pub async fn debounce<F>(&self, f: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut handle = self.handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
        let delay = self.delay;
        *handle = Some(tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            f.await;
        }));
    }
}

impl Clone for Debouncer {
    fn clone(&self) -> Self {
        Self {
            handle: Arc::clone(&self.handle),
            delay: self.delay,
        }
    }
}
