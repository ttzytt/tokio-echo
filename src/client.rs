use tokio::net::TcpStream;
use crate::{common::{Config, MessageHandler, BoxError}, session_starter::SessionStarter};
use tokio::span;
use std::sync::Arc;

pub struct Client {
    cfg: Config,
    addr: String,
    handlers: Vec<Arc<dyn MessageHandler>>,
}

impl Client {
    pub fn new(cfg: Config, addr: &str) -> Self {
        Self { cfg, addr: addr.into(), handlers: Vec::new() }
    }

    pub fn register(&mut self, handler: Arc<dyn MessageHandler>) {
        self.handlers.push(handler);
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        if self.cfg.use_mux {
            let socket = TcpStream::connect(&self.addr).await?;
            SessionStarter::start_mux(socket, self.handlers.clone(), self.cfg.clone());
        } else {
            for (idx, handler) in self.handlers.iter().enumerate() {
                let socket = TcpStream::connect(&self.addr).await?;
                let id = (idx + 1) as u32;
                SessionStarter::start_simple(socket, id, self.cfg.clone(), handler.clone());
            }
        }
        Ok(())
    }
}
