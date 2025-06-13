use crate::{
    common::{BoxError, ClientHandler, Config, RxIn, RxOut, TxIn, TxOut},
    frame::Frame,
    session::{ClientSession, RawSession},
    transport::{
        readers::{ReaderTxInOpt, frame_reader_task},
        writers::writer_task,
    },
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncRead, split};
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc::unbounded_channel};
use tokio::task::JoinHandle;

use tracing::Instrument;

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

    pub async fn run(&self) {
        if self.cfg.use_mux {
            let sock = TcpStream::connect(&self.addr).await.unwrap();
            let _ = sock.set_nodelay(self.cfg.batch.is_some());
            Client::start_mux_client(sock, self.handlers.clone(), self.cfg.clone()).await
        } else {
            let mut handles: Vec<JoinHandle<()>> = Vec::new();
            for h in self.handlers.iter() {
                let sock = TcpStream::connect(&self.addr).await.unwrap();
                let _ = sock.set_nodelay(self.cfg.batch.is_some());
                handles.push(Client::start_simple_client(
                    sock,
                    self.cfg.clone(),
                    h.clone(),
                ));
            }

            for h in handles {
                h.await.unwrap();
            }
        }
    }

    /// Simple TCP (no mux) for clients
    fn start_simple_client(
        socket: TcpStream,
        cfg: Config,
        handler: Arc<dyn ClientHandler>,
    ) -> JoinHandle<()> {
        let (r, w) = split(socket);
        let (raw, rx_out, tx_in) = RawSession::new();
        let stop_sig = Arc::new(Notify::new());
        let writer_handle = tokio::spawn(
            writer_task(w, rx_out, cfg.batch.clone(), Some(stop_sig.clone()))
                .instrument(tracing::info_span!("writer_task", is_server = false)),
        );

        let reader_handle = tokio::spawn(
            frame_reader_task(r, ReaderTxInOpt::TxIn(tx_in), Some(stop_sig.clone()))
                .instrument(tracing::info_span!("reader_task", is_server = false)),
        );
        
        let ch_join_handle = tokio::spawn(async move {
            // ch = client handler
            let mut sess = ClientSession::new(raw, 0);
            handler.run(&mut sess).await;
        });

        tokio::spawn(async move {
            ch_join_handle.await.unwrap();
            stop_sig.notify_one();
            stop_sig.notify_one();
            reader_handle.await.unwrap().unwrap();
            writer_handle.await.unwrap();
        })
    }

    /// Mux‐client entry point: each registered ClientHandler drives one substream ID
    async fn start_mux_client(
        socket: TcpStream,
        handlers: Vec<Arc<dyn ClientHandler>>,
        cfg: Config,
    ) {
        let (r, w) = split(socket);
        let (raw, rx_out, _tx_in) = RawSession::new();
        let stop_sig = Arc::new(Notify::new());
        let writer_handle = tokio::spawn(writer_task(
            w,
            rx_out,
            cfg.batch.clone(),
            Some(stop_sig.clone()),
        ));
        let (ch_join_handle, id_to_tx_in) =
            Client::start_mux_client_handlers(raw.tx_out.clone(), handlers).await;
        let reader_handle = tokio::spawn(frame_reader_task(
            r,
            ReaderTxInOpt::IdToTxIn(id_to_tx_in),
            Some(stop_sig.clone()),
        ));

        ch_join_handle.await.unwrap();
        stop_sig.notify_one();
        stop_sig.notify_one();
        reader_handle.await.unwrap().unwrap();
        writer_handle.await.unwrap();
    }

    pub async fn start_mux_client_handlers(
        common_tx_out: TxOut,
        handlers: Vec<Arc<dyn ClientHandler>>,
    ) -> (JoinHandle<()>, HashMap<u32, TxIn>) {
        let max_id = handlers.len() as u32;
        let mut sessions: HashMap<u32, TxIn> = HashMap::new();
        let mut ch_join_handles: Vec<JoinHandle<()>> = Vec::new();
        // ch = client handler
        for id in 1..=max_id {
            let (tx_in, rx_in): (TxIn, RxIn) = unbounded_channel();
            sessions.insert(id, tx_in.clone());

            let raw_sub = RawSession {
                tx_out: common_tx_out.clone(),
                rx_in,
            };
            let handler = handlers[(id - 1) as usize].clone();
            ch_join_handles.push(tokio::spawn(async move {
                let mut sess = ClientSession::new(raw_sub, id);
                handler.run(&mut sess).await;
            }));
        }
        (
            tokio::spawn(async move {
                for h in ch_join_handles {
                    h.await.unwrap();
                }
            }),
            sessions,
        )
    }
}
