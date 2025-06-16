// src/server.rs

use crate::common::{Amrc, BoxError, ByteSeq, Config, Id_t, ServerHandler};
use crate::frame::{Frame, FrameKind};
use crate::session::{RawSession, ServerSession};
use crate::transport::readers::{ReaderTxInOpt, frame_reader_task};
use crate::transport::writers::writer_task;
use dashmap::{DashMap, DashSet};
use std::sync::Arc;
use tokio::{
    io::split,
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
    task::JoinHandle,
};
use tracing::Instrument;

/// Wraps all per-connection state in Simple mode.
struct Conn {
    raw: RawSession,
    stop_sig: Arc<Notify>,
    reader: JoinHandle<Result<(), BoxError>>,
    writer: JoinHandle<()>,
}

/// SimpleServerSession: one Conn per TCP client, stored in a DashMap.
pub struct SimpleServerSession {
    conns: Arc<DashMap<Id_t, Conn>>,
}

impl SimpleServerSession {
    /// Create a new, empty session map.
    pub fn new() -> Self {
        Self {
            conns: Arc::new(DashMap::new()),
        }
    }

    /// Register a new TCP connection under the provided `id`.
    pub async fn reg_new_session(&self, sock: TcpStream, id: Id_t, cfg: Config) {
        let (r, w) = split(sock);
        let (raw, rx_out, tx_in) = RawSession::new();
        let stop_sig = Arc::new(Notify::new());

        // Spawn writer task for this connection.
        let writer_handle = tokio::spawn(
            writer_task(w, rx_out, cfg.batch.clone(), Some(stop_sig.clone())).instrument(
                tracing::info_span!("writer_task", is_server = true, id = id),
            ),
        );

        // Spawn reader task with Terminate callback that removes just this Conn.
        let stop_clone = stop_sig.clone();
        let map_clone = self.conns.clone();
        let reader_handle = tokio::spawn(
            frame_reader_task(
                r,
                ReaderTxInOpt::TxIn(tx_in.clone()),
                Some(stop_clone.clone()),
                Some(move |frame: &Frame| {
                    if frame.kind == FrameKind::TerminateAll {
                        // Remove this connection and signal its tasks to stop.
                        map_clone.remove(&id);
                        stop_clone.notify_one();
                        stop_clone.notify_one();
                    }
                }),
            )
            .instrument(tracing::info_span!(
                "reader_task",
                is_server = true,
                id = id
            )),
        );

        // Insert the new Conn into the map.
        self.conns.insert(
            id,
            Conn {
                raw,
                stop_sig,
                reader: reader_handle,
                writer: writer_handle,
            },
        );
    }

    /// Gracefully shut down all active connections.
    pub async fn shutdown(&self) {
        // Collect all current IDs.
        let ids: Vec<Id_t> = self.conns.iter().map(|e| *e.key()).collect();
        for id in ids {
            // Remove the Conn from the map, taking ownership.
            if let Some((_, conn)) = self.conns.remove(&id) {
                // Notify its reader and writer to stop.
                conn.stop_sig.notify_one();
                conn.stop_sig.notify_one();
                // Await their completion.
                let _ = conn.reader.await.unwrap();
                let _ = conn.writer.await.unwrap();
            }
        }
    }
}

#[async_trait::async_trait]
impl ServerSession for SimpleServerSession {
    async fn recv(&mut self) -> Option<(Id_t, ByteSeq)> {
        for mut entry in self.conns.iter_mut() {
            let id = *entry.key();
            if let Ok(frame) = entry.value_mut().raw.rx_in.try_recv() {
                return Some((id, frame.payload));
            }
        }
        None
    }

    fn send_to(&self, id: Id_t, payload: ByteSeq) {
        if let Some(conn_ref) = self.conns.get(&id) {
            let _ = conn_ref.raw.tx_out.send(Frame {
                kind: FrameKind::Data,
                id,
                payload,
            });
        }
    }

    fn is_connected(&self, id: Id_t) -> bool {
        self.conns.contains_key(&id)
    }

    fn client_count(&self) -> usize {
        self.conns.len()
    }
}

/// MuxServerSession: single RawSession + a set of active substream IDs.
pub struct MuxServerSession {
    raw: RawSession,
    active_ids: Arc<DashSet<Id_t>>,
}

impl MuxServerSession {
    pub fn new(raw: RawSession, active_ids: Arc<DashSet<Id_t>>) -> Self {
        Self { raw, active_ids }
    }
}

#[async_trait::async_trait]
impl ServerSession for MuxServerSession {
    async fn recv(&mut self) -> Option<(Id_t, ByteSeq)> {
        self.raw
            .rx_in
            .recv()
            .await
            .map(|f| (f.id, f.payload))
            .filter(|(id, _)| self.active_ids.contains(id))
    }

    fn send_to(&self, id: Id_t, payload: ByteSeq) {
        let _ = self.raw.tx_out.send(Frame {
            kind: FrameKind::Data,
            id,
            payload,
        });
    }

    fn is_connected(&self, id: Id_t) -> bool {
        self.active_ids.contains(&id)
    }

    fn client_count(&self) -> usize {
        self.active_ids.len()
    }
}

/// Top‐level server: chooses between Simple and Mux modes.
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
            Self::start_mux_server(sock, self.handler.clone(), self.cfg.clone()).await;
        } else {
            let session = Amrc::new(Mutex::new(SimpleServerSession::new()));
            let handler = self.handler.clone();
            let session_for_handler = session.clone();
            let cfg = self.cfg.clone();
            let stop_accept = self.stop_accept_sig.clone();

            let handler_task = tokio::spawn(async move {
                handler.run(session_for_handler).await;
            });

            let session_for_accept = session.clone();

            let accept_task = tokio::spawn(async move {
                let mut next_id: Id_t = 1;
                loop {
                    tokio::select! {
                        _ = stop_accept.notified() => return,
                        res = listener.accept() => match res {
                            Ok((sock, _)) => {
                                let _ = sock.set_nodelay(cfg.batch.is_some());
                                let session = session_for_accept.lock().await;
                                session.reg_new_session(sock, next_id, cfg.clone()).await;
                                next_id += 1;
                            }
                            Err(e) => tracing::error!("accept error: {}", e),
                        },
                    }
                }
            });

            accept_task.await.unwrap();
            handler_task.await.unwrap();

            let session = session.lock().await;
            session.shutdown().await;
        }
    }

    async fn start_mux_server(socket: TcpStream, handler: Arc<dyn ServerHandler>, cfg: Config) {
        let (r, w) = split(socket);
        let (raw_sess, rx_out, tx_in) = RawSession::new();
        let stop_sig = Arc::new(Notify::new());
        let active_ids = Arc::new(DashSet::new());

        let writer_handle = tokio::spawn(
            writer_task(w, rx_out, cfg.batch.clone(), Some(stop_sig.clone()))
                .instrument(tracing::info_span!("writer_task", is_server = true)),
        );

        let ids_cb = active_ids.clone();
        let stop_sig_for_spawn = stop_sig.clone();
        let reader_handle = tokio::spawn(
            frame_reader_task(
                r,
                ReaderTxInOpt::TxIn(tx_in),
                Some(stop_sig_for_spawn.clone()),
                Some(move |frame: &Frame| {
                    let mut stop_tasks = false;
                    if frame.kind == FrameKind::TerminateAll {
                        ids_cb.clear();
                        stop_tasks = true;
                    } else if frame.kind == FrameKind::TerminateId {
                        ids_cb.remove(&frame.id);
                        if ids_cb.is_empty() {
                            // TODO: this will not allow new clients to connect
                            stop_tasks = true;
                        }
                    } else {
                        ids_cb.insert(frame.id);
                    }
                    if stop_tasks {
                        stop_sig_for_spawn.notify_one();
                        stop_sig_for_spawn.notify_one();
                    }
                }),
            )
            .instrument(tracing::info_span!("reader_task", is_server = true)),
        );

        let mux_sess = MuxServerSession::new(raw_sess, active_ids.clone());
        handler.run(Amrc::from(Mutex::from(mux_sess))).await;

        stop_sig.notify_one();
        stop_sig.notify_one();
        let _ = reader_handle.await.unwrap();
        let _ = writer_handle.await.unwrap();
    }
}
