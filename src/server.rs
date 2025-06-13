use crate::{
    common::{Amrc, BoxError, Config, ServerHandler},
    frame::Frame,
    session::{RawSession, ServerSession},
    transport::{
        readers::{ReaderTxInOpt, frame_reader_task},
        writers::writer_task,
    },
};
use std::sync::Arc;
use tokio::io::split;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tracing::{Instrument};

pub struct MuxServerSession {
    raw: RawSession,
}

#[async_trait::async_trait]
impl ServerSession for MuxServerSession {
    async fn recv(&mut self) -> Option<(u32, Vec<u8>)> {
        self.raw.rx_in.recv().await.map(|f| (f.id, f.payload))
    }

    fn send_to(&self, sub_id: u32, payload: Vec<u8>) {
        let _ = self.raw.tx_out.send(Frame {
            id: sub_id,
            payload,
        });
    }
}

impl MuxServerSession {
    pub fn new(raw: RawSession) -> Self {
        Self { raw }
    }
}

pub struct SimpleServerSession {
    tcp_sessions: Vec<RawSession>,
    stop_sig: Arc<Notify>,
    reader_handles: Vec<JoinHandle<Result<(), BoxError>>>,
    writer_handles: Vec<JoinHandle<()>>,
}

#[async_trait::async_trait]
impl ServerSession for SimpleServerSession {
    async fn recv(&mut self) -> Option<(u32, Vec<u8>)> {
        for (i, session) in self.tcp_sessions.iter_mut().enumerate() {
            if let Ok(frame) = session.rx_in.try_recv() {
                return Some(((i + 1) as u32, frame.payload));
            }
        }
        None
    }

    fn send_to(&self, id: u32, payload: Vec<u8>) {
        let single_sess = &self.tcp_sessions[id as usize - 1];
        single_sess.tx_out.send(Frame { id: 0, payload }).unwrap();
    }
}

impl SimpleServerSession {
    pub fn reg_new_session(&mut self, sock: TcpStream, id: u32, cfg: Config) {
        let (r, w) = split(sock);
        let (raw, rx_out, tx_in) = RawSession::new();

        self.writer_handles.push(tokio::spawn(
            writer_task(w, rx_out, cfg.batch.clone(), Some(self.stop_sig.clone())).instrument(
                tracing::info_span!("writer_task", is_server = true, id = id),
            ),
        ));

        self.reader_handles.push(tokio::spawn(
            frame_reader_task(
                r,
                ReaderTxInOpt::TxIn(tx_in.clone()),
                Some(self.stop_sig.clone()),
            )
            .instrument(tracing::info_span!(
                "reader_task",
                is_server = true,
                id = id
            )),
        ));

        self.tcp_sessions.push(raw);
        assert!(self.tcp_sessions.len() == id as usize);
    }

    pub async fn stop_rw_tasks(&mut self) {
        for (r_handle, w_handle) in self
            .reader_handles
            .drain(..)
            .zip(self.writer_handles.drain(..))
        {
            self.stop_sig.notify_one();
            self.stop_sig.notify_one();
            r_handle.await.unwrap().unwrap();
            w_handle.await.unwrap();
        }
    }

    pub fn new() -> Self {
        Self {
            tcp_sessions: Vec::new(),
            stop_sig: Arc::new(Notify::new()),
            reader_handles: Vec::new(),
            writer_handles: Vec::new(),
        }
    }
}

pub struct Server {
    cfg: Config,
    handler: Arc<dyn ServerHandler>,
    addr: String,
    stop_accept_sig: Arc<Notify>,
}

impl Server {
    pub fn new(cfg: Config, handler: Arc<dyn ServerHandler>, addr: &str) -> Self {
        Self {
            cfg,
            handler,
            addr: addr.into(),
            stop_accept_sig: Arc::new(Notify::new()),
        }
    }

    pub fn stop_accept(&self) {
        self.stop_accept_sig.notify_one();
    }

    pub async fn run(&self) {
        let listener = TcpListener::bind(&self.addr).await.unwrap();
        if self.cfg.use_mux {
            let (sock, _) = listener.accept().await.unwrap();
            let _ = sock.set_nodelay(self.cfg.batch.is_some());
            Server::start_mux_server(sock, self.handler.clone(), self.cfg.clone())
                .await
                .unwrap();
        } else {
            let mut next_id = 1;
            let simple_session = Amrc::new(Mutex::new(SimpleServerSession::new()));
            let handler = self.handler.clone();
            let sess_for_spawn = simple_session.clone();
            let sess_for_accept = simple_session.clone();
            let cfg = self.cfg.clone();

            let sh_join_handle = tokio::spawn(async move {
                handler.run(sess_for_spawn).await;
            });

            let stop_accept_sig = self.stop_accept_sig.clone();
            let accept_handle = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        biased;
                        _ = stop_accept_sig.notified() => {
                            return;
                        }
                        res = listener.accept() => {
                            match res {
                                Ok((sock, _)) => {
                                    let _ = sock.set_nodelay(cfg.batch.is_some());
                                    let mut sess = sess_for_accept.lock().await;
                                    sess.reg_new_session(sock, next_id, cfg.clone());
                                    next_id += 1;
                                },
                                Err(e) => {
                                    eprintln!("Error accepting connection: {}", e);
                                }
                            }
                        },
                    }
                }
            });

            accept_handle.await.unwrap();
            sh_join_handle.await.unwrap();
            simple_session.lock().await.stop_rw_tasks().await;
        }
    }

    fn start_mux_server(
        socket: TcpStream,
        handler: Arc<dyn ServerHandler>,
        cfg: Config,
    ) -> JoinHandle<()> {
        let (r, w) = split(socket);
        let (raw_sess, rx_out, tx_in) = RawSession::new();

        let stop_sig = Arc::new(Notify::new());
        let writer_handle = tokio::spawn(
            writer_task(w, rx_out, cfg.batch.clone(), Some(stop_sig.clone()))
                .instrument(tracing::info_span!("writer_task", is_server = true,)),
        );
        let reader_handle = tokio::spawn(
            frame_reader_task(r, ReaderTxInOpt::TxIn(tx_in), Some(stop_sig.clone()))
                .instrument(tracing::info_span!("reader_task", is_server = true)),
        );
        let sh_join_handle = tokio::spawn(async move {
            let serv_sess = MuxServerSession::new(raw_sess);
            handler.run(Amrc::from(Mutex::from(serv_sess))).await;
        });

        tokio::spawn(async move {
            sh_join_handle.await.unwrap();
            stop_sig.notify_one();
            stop_sig.notify_one();
            reader_handle.await.unwrap().unwrap();
            writer_handle.await.unwrap();
        })
    }
}
