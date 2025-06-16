use crate::{
    common::{ClientHandler, Config, Id_t, RxIn_t, TxIn_t, TxOut_t},
    frame::Frame,
    session::{ClientSession, RawSession},
    transport::{
        readers::{ReaderTxInOpt, frame_reader_task},
        writers::writer_task,
    },
};

use std::collections::HashMap;
use std::sync::Arc;
use tracing::Instrument;
use tokio::{
    io::{split},
    net::TcpStream,
    sync::{Notify, mpsc::unbounded_channel},
    task::JoinHandle,
};

pub struct Client {
    cfg: Config,
    addr: String,
    handlers: Vec<Arc<dyn ClientHandler>>,
    // TODO: change to Arc<Dashset> so that we can register and remove handlers dynamically
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
            frame_reader_task(
                r,
                ReaderTxInOpt::TxIn(tx_in),
                Some(stop_sig.clone()),
                None::<fn(&Frame)>,
            )
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
            None::<fn(&Frame)>,
        ));

        ch_join_handle.await.unwrap();
        stop_sig.notify_one();
        stop_sig.notify_one();
        reader_handle.await.unwrap().unwrap();
        writer_handle.await.unwrap();
    }

    pub async fn start_mux_client_handlers(
        common_tx_out: TxOut_t,
        handlers: Vec<Arc<dyn ClientHandler>>,
    ) -> (JoinHandle<()>, HashMap<Id_t, TxIn_t>) {
        // TODO: this does not allow adding new handlers once the client manager is running
        let max_id = handlers.len() as Id_t;
        let mut sessions: HashMap<Id_t, TxIn_t> = HashMap::new();
        let mut ch_join_handles: Vec<JoinHandle<()>> = Vec::new();
        // ch = client handler
        for id in 1..=max_id {
            let (tx_in, rx_in): (TxIn_t, RxIn_t) = unbounded_channel();
            sessions.insert(id, tx_in.clone());

            let raw_sub = RawSession {
                tx_out: common_tx_out.clone(),
                rx_in,
            };
            let handler = handlers[(id - 1) as usize].clone();
            let common_tx_out = common_tx_out.clone();
            ch_join_handles.push(tokio::spawn(async move {
                let mut sess = ClientSession::new(raw_sub, id);
                handler.run(&mut sess).await;
                common_tx_out.send(Frame::terminate_id(id)).unwrap();
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
