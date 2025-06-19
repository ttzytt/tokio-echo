use crate::{frame::Frame, session};
use async_trait::async_trait;
use std::time::Duration;
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

pub type BoxError = Box<dyn Error + Send + Sync>;
pub type Amrc<T> = Arc<Mutex<T>>;
pub type TxOut_t = tokio::sync::mpsc::UnboundedSender<Frame>;
pub type RxOut_t = tokio::sync::mpsc::UnboundedReceiver<Frame>;
pub type TxIn_t = tokio::sync::mpsc::UnboundedSender<Frame>;
pub type RxIn_t = tokio::sync::mpsc::UnboundedReceiver<Frame>;
pub type Id_t = u32;
pub type ByteSeq = Vec<u8>;

/// Batching configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchConfig {
    pub size_byte: usize,
    pub delay: Duration,
}

/// Global configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransportConfig {
    pub use_mux: bool,
    pub batch: Option<BatchConfig>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TransportConfig{
    pub const DEFAULT : Self = Self {
        use_mux: false,
        batch: None,
    };
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
