use tokio::net::TcpStream;
use std::sync::Arc;
use crate::{
    common::{Config, ClientHandler, BoxError},
    session_starter::SessionStarter,
};

pub struct Client {
    cfg: Config,
    addr: String,
    handlers: Vec<Arc<dyn ClientHandler>>,
}

impl Client {
    pub fn new(cfg: Config, addr: &str) -> Self {
        Self { cfg, addr: addr.into(), handlers: Vec::new() }
    }

    pub fn register(&mut self, handler: Arc<dyn ClientHandler>) {
        self.handlers.push(handler);
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        if self.cfg.use_mux {
            let sock = TcpStream::connect(&self.addr).await?;
            SessionStarter::start_mux_client(sock, self.handlers.clone(), self.cfg.clone());
        } else {
            for (i, h) in self.handlers.iter().enumerate() {
                let sock = TcpStream::connect(&self.addr).await?;
                SessionStarter::start_simple_client(sock, (i + 1) as u32, self.cfg.clone(), h.clone());
            }
        }
        Ok(())
    }
}
