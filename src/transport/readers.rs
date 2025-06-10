use tokio::io::{AsyncBufReadExt, AsyncRead};
use tokio::sync::mpsc::UnboundedSender;
use crate::frame::read_frame;

pub async fn simple_reader_task<R>(
    reader: R,
    mut tx_in: UnboundedSender<Vec<u8>>,
) where
    R: AsyncRead + Unpin,
{
    let mut lines = tokio::io::BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = tx_in.send(line.into_bytes());
    }
}


pub async fn mux_reader_task<R>(
    mut reader: R,
    mut tx_in: UnboundedSender<Vec<u8>>,
    valid_ids: &[u32],
) where
    R: AsyncRead + Unpin,
{
    while let Ok(frame) = read_frame(&mut reader).await {
        if valid_ids.contains(&frame.id) {
            let _ = tx_in.send(frame.payload);
        } else {
            eprintln!("[MuxReader] invalid id={}", frame.id);
        }
    }
}