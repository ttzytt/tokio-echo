use std::{error::Error, sync::Arc};
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::time::Duration;

pub type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
pub struct BatchConfig {
    pub size: usize,
    pub delay: Duration,
}

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


#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    async fn run(&self, ctx: &mut crate::session::SessionContext);
}
