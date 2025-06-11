use crate::frame::{Frame, read_frame};
use tokio::io::{AsyncRead};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::common::ClientHandler;
use crate::session::{ClientSession, RawSession};
use std::collections::HashMap;
use std::sync::Arc;


pub async fn frame_reader_task<R>(mut reader: R, tx_in: UnboundedSender<Frame>)
where
    R: AsyncRead + Unpin,
{
    while let Ok(frame) = read_frame(&mut reader).await {
        let _ = tx_in.send(frame);
    }
}

pub async fn mux_client_reader_task<R>(
    mut reader: R,
    raw_tx: UnboundedSender<Frame>,
    handlers: Vec<Arc<dyn ClientHandler>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    let max_id = handlers.len() as u32;
    let mut sessions: HashMap<u32, UnboundedSender<Frame>> = HashMap::new();
    for id in 1..=max_id {
        let (tx, rx): (UnboundedSender<Frame>, UnboundedReceiver<Frame>) = unbounded_channel();
        sessions.insert(id, tx.clone());

        let raw_sub = RawSession {
            tx_out: raw_tx.clone(),
            rx_in: rx,
        };
        let handler = handlers[(id - 1) as usize].clone();
        tokio::spawn(async move {
            let mut sess = ClientSession::new(raw_sub, id);
            handler.run(&mut sess).await;
        });
    }

    while let Ok(frame) = read_frame(&mut reader).await {
        let id = frame.id;
        if id == 0 || id > max_id {
            eprintln!("[readers] invalid id={}", id);
            continue;
        }
        if let Some(tx) = sessions.get(&id) {
            let _ = tx.send(frame);
        }
    }
}
