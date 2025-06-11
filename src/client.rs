use crate::{
    common::{BoxError, ClientHandler, Config},
    session::{ClientSession, RawSession},
    transport::{
        readers::{frame_reader_task, mux_client_reader_task},
        writers::writer_task,
    },
};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::split;

pub struct Client {
    cfg: Config,
    addr: String,
    handlers: Vec<Arc<dyn ClientHandler>>,
}

impl Client {
    pub fn new(cfg: Config, addr: &str) -> Self {
        Self {
            cfg,
            addr: addr.into(),
            handlers: Vec::new(),
        }
    }

    pub fn register(&mut self, handler: Arc<dyn ClientHandler>) {
        self.handlers.push(handler);
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        if self.cfg.use_mux {
            let sock = TcpStream::connect(&self.addr).await?;
            Client::start_mux_client(sock, self.handlers.clone(), self.cfg.clone());
        } else {
            for h in self.handlers.iter() {
                let sock = TcpStream::connect(&self.addr).await?;
                Client::start_simple_client(sock, self.cfg.clone(), h.clone());
            }
        }
        Ok(())
    }

    /// Simple TCP (no mux) for clients
    fn start_simple_client(socket: TcpStream, cfg: Config, handler: Arc<dyn ClientHandler>) {
        let (r, w) = split(socket);
        let (raw, rx_out, tx_in) = RawSession::new();
        tokio::spawn(writer_task(w, rx_out, cfg.batch.clone()));
        tokio::spawn(frame_reader_task(r, tx_in));
        tokio::spawn(async move {
            let mut sess = ClientSession::new(raw, 0);
            handler.run(&mut sess).await;
        });
    }

    /// Mux‐client entry point: each registered ClientHandler drives one substream ID
    fn start_mux_client(socket: TcpStream, handlers: Vec<Arc<dyn ClientHandler>>, cfg: Config) {
        let (r, w) = split(socket);
        let (raw, rx_out, _tx_in) = RawSession::new();
        tokio::spawn(writer_task(w, rx_out, cfg.batch.clone()));
        tokio::spawn(mux_client_reader_task(r, raw.tx_out, handlers));
    }
}
