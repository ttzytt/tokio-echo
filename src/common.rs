use std::error::Error;
use std::sync::Arc;
use futures::future::BoxFuture;
pub type BoxError = Box<dyn Error + Send + Sync + 'static>;
pub type Handler = Arc<dyn Fn(u32, Vec<u8>) -> BoxFuture<'static, Vec<u8>> + Send + Sync>;


#[derive(Clone)]
pub struct Config {
    pub threads: usize,
    pub batching: bool,
    pub batch_size: usize,
    pub batch_delay_ms: u64,
    pub use_mux: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threads: num_cpus::get(),
            batching: false,
            batch_size: 10,
            batch_delay_ms: 100,
            use_mux: false,
        }
    }
}