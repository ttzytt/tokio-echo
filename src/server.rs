use tokio::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering};
use crate::{common::{Config, MessageHandler, BoxError}, session_starter::SessionStarter};
use tokio::spawn;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Server {
    cfg: Config,
    handler: Arc<dyn MessageHandler>,
    addr: String,
}

impl Server {
    pub fn new(cfg: Config, handler: Arc<dyn MessageHandler>, addr: &str) -> Self {
        Self { cfg, handler, addr: addr.into() }
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        let listener = TcpListener::bind(&self.addr).await?;
        println!("Server listening on {}", self.addr);

        if self.cfg.use_mux {
            let (socket, _) = listener.accept().await?;
            SessionStarter::start_mux(socket, vec![self.handler.clone()], self.cfg.clone());
        } else {
            let counter = AtomicU32::new(1);
            loop {
                let (socket, _) = listener.accept().await?;
                let id = counter.fetch_add(1, Ordering::SeqCst);
                SessionStarter::start_simple(socket, id, self.cfg.clone(), self.handler.clone());
            }
        }
        Ok(())
    }
}
