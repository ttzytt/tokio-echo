use crate::{
    common::{ClientHandler, Config, ServerHandler, Amrc},
    frame::Frame,
    session::{ClientSession, RawSession, ServerSession},
    transport::{
        readers::{mux_client_reader_task, frame_reader_task},
        writers::writer_task,
    },
    server::MuxServerSession,
};
use core::hash;
use std::sync::Arc;
use tokio::{io::split, net::TcpStream};
use tokio::sync::Mutex;


/// Simple TCP (no mux) for clients
pub fn start_simple_client(
    socket: TcpStream,
    cfg: Config,
    handler: Arc<dyn ClientHandler>,
) {
    let (r, w) = split(socket);
    let (raw, rx_out, tx_in) = RawSession::new();
    tokio::spawn(writer_task(w, rx_out, cfg.batch.clone()));
    tokio::spawn(frame_reader_task(r, tx_in));
    tokio::spawn(async move {
        let mut sess = ClientSession::new(raw, 0);
        handler.run(&mut sess).await;
    });
}



/// Mux‐server entry point: a single ServerHandler handles *all* substream IDs
pub fn start_mux_server(socket: TcpStream, handler: Arc<dyn ServerHandler>, cfg: Config) {
    let (r, w) = split(socket);
    let (raw_sess, rx_out, tx_in) = RawSession::new();

    // 1) spawn the write loop
    tokio::spawn(writer_task(w, rx_out, cfg.batch.clone()));
    tokio::spawn(frame_reader_task(r, tx_in));
    tokio::spawn(async move {
        let serv_sess = MuxServerSession::new(raw_sess);
        handler.run(Amrc::from(Mutex::from(serv_sess))).await;
    });
}

/// Mux‐client entry point: each registered ClientHandler drives one substream ID
pub fn start_mux_client(socket: TcpStream, handlers: Vec<Arc<dyn ClientHandler>>, cfg: Config) {
    let (r, w) = split(socket); 
    let (raw, rx_out, tx_in) = RawSession::new();
    tokio::spawn(writer_task(w, rx_out, cfg.batch.clone()));
    tokio::spawn(mux_client_reader_task(r, raw.tx_out, handlers));

}
