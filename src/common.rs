use std::{error::Error, sync::Arc};
use async_trait::async_trait;
use std::time::Duration;
use crate::session;

pub type BoxError = Box<dyn Error + Send + Sync>;

/// Batching configuration
#[derive(Clone)]
pub struct BatchConfig {
    pub size: usize,
    pub delay: Duration,
}

/// Global configuration
#[derive(Clone)]
pub struct Config {
    pub threads: usize,
    pub use_mux: bool,
    pub batch: Option<BatchConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threads: num_cpus::get(),
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
    async fn run(&self, sess: &mut session::ServerSession);
}
