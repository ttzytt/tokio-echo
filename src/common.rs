use std::{error::Error, sync::Arc};
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::Mutex;
use crate::session;

pub type BoxError = Box<dyn Error + Send + Sync>;
pub type Amrc<T> = Arc<Mutex<T>>;

/// Batching configuration
#[derive(Clone)]
pub struct BatchConfig {
    pub size: usize,
    pub delay: Duration,
}

/// Global configuration
#[derive(Clone)]
pub struct Config {
    pub use_mux: bool,
    pub batch: Option<BatchConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            use_mux: false,
            batch: None,
        }
    }
}

/// Client‐side handler: sees only payloads, no stream IDs
#[async_trait]
pub trait ClientHandler: Send + Sync + 'static {
    async fn run(&self, sess: &mut session::ClientSession);
}

/// Server‐side handler: sees stream IDs + payloads
#[async_trait]
pub trait ServerHandler: Send + Sync + 'static {
    async fn run(&self, mut sess: Amrc<dyn session::ServerSession + Send>);
}
