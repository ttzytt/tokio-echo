use tokio::net::TcpStream;
use std::sync::Arc;
use crate::{
    common::{Config, ClientHandler, BoxError},
    session_starter,
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
            session_starter::start_mux_client(sock, self.handlers.clone(), self.cfg.clone());
        } else {
            for h in self.handlers.iter() {
                let sock = TcpStream::connect(&self.addr).await?;
                session_starter::start_simple_client(sock, self.cfg.clone(), h.clone());
            }
        }
        Ok(())
    }
}
