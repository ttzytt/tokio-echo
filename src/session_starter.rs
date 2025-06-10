use std::collections::HashMap;

use tokio::{net::TcpStream, io::split, spawn};
use tokio::sync::mpsc::UnboundedSender;
use crate::{
    common::{Config, MessageHandler},
    session::{SessionCore, SessionContext},
    frame::Frame,
    transport::{writers::writer_task, readers::simple_reader_task, readers::mux_reader_task},
};
use std::sync::Arc;

pub struct SessionStarter;

impl SessionStarter {
    pub fn start_simple(
        socket: TcpStream,
        id: u32,
        cfg: Config,
        handler: Arc<dyn MessageHandler>,
    ) {
        let (r, w) = split(socket);
        let (core, rx_out, tx_in) = SessionCore::new(id, cfg.clone());

        spawn(writer_task(w, rx_out, cfg.batch.clone()));
        spawn(simple_reader_task(r, tx_in));
        spawn(async move {
            let mut ctx = SessionContext::from(core);
            handler.run(&mut ctx).await;
        });
    }

    pub fn start_mux(
        socket: TcpStream,
        handlers: Vec<Arc<dyn MessageHandler>>,
        cfg: Config,
    ) {
        let (r, w) = split(socket);
        let (tx_out, rx_out) = tokio::sync::mpsc::unbounded_channel::<Frame>();

        spawn(writer_task(w, rx_out, cfg.batch.clone()));

        let valid_ids: Vec<u32> = (1..=handlers.len() as u32).collect();
        let mut sessions: HashMap<u32, UnboundedSender<Vec<u8>>> = HashMap::new();
        spawn(async move {
            let mut reader = r;
            while let Ok(frame) = crate::frame::read_frame(&mut reader).await {
                let id = frame.id;
                if let Some(tx_in) = sessions.get(&id) {
                    let _ = tx_in.send(frame.payload);
                } else if valid_ids.contains(&id) {
                    let (tx_in, rx_in) = tokio::sync::mpsc::unbounded_channel();
                    let _ = tx_in.send(frame.payload);
                    sessions.insert(id, tx_in.clone());
                    let tx_out_cl = tx_out.clone();
                    let mut ctx = SessionContext::new(id, tx_out_cl, rx_in);
                    let handler = handlers[(id as usize) - 1].clone();

                    spawn(async move {
                        handler.run(&mut ctx).await;
                    });
                } else {
                    eprintln!("[MuxSession] invalid sub-stream id={}", id);
                }
            }
        });
    }
}
