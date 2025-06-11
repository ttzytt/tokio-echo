use tokio::net::TcpListener;
use std::sync::Arc;
use crate::{
    common::{Config, ServerHandler, BoxError},
    session_starter::SessionStarter,
};

pub struct Server {
    cfg: Config,
    handler: Arc<dyn ServerHandler>,
    addr: String,
}

impl Server {
    pub fn new(cfg: Config, handler: Arc<dyn ServerHandler>, addr: &str) -> Self {
        Self { cfg, handler, addr: addr.into() }
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        let listener = TcpListener::bind(&self.addr).await?;
        println!("Server listening on {}", self.addr);

        if self.cfg.use_mux {
            let (sock, _) = listener.accept().await?;
            SessionStarter::start_mux_server(sock, self.handler.clone(), self.cfg.clone());
        } else {
            let mut next_id = 1;
            loop {
                let (sock, _) = listener.accept().await?;
                SessionStarter::start_simple_server(sock, next_id, self.cfg.clone(), self.handler.clone());
                next_id += 1;
            }
        }
        Ok(())
    }
}
