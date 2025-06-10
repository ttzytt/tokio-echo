use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedReceiver;
use crate::{common::BatchConfig, frame::{Frame, write_frame}};

pub async fn writer_task<W>(
    mut writer: W,
    mut rx_out: UnboundedReceiver<Frame>,
    batch: Option<BatchConfig>,
) where
    W: AsyncWriteExt + Unpin,
{
    if let Some(BatchConfig { size, delay }) = batch {
        // if batching is enabled
        let mut buf = Vec::with_capacity(size);
        let mut last = tokio::time::Instant::now();
        while let Some(frame) = rx_out.recv().await {
            buf.push(frame);
            if buf.len() >= size {
                flush(&mut writer, &mut buf).await;
                last = tokio::time::Instant::now();
            }
            tokio::select! {
                _ = tokio::time::sleep_until(last + delay) => {
                    if !buf.is_empty() {
                        flush(&mut writer, &mut buf).await;
                        last = tokio::time::Instant::now();
                    }
                }
                else => {}
            }
        }
    } else {
        // just write frames as they come without batching
        while let Some(frame) = rx_out.recv().await {
            let _ = write_frame(&mut writer, &frame).await; 
            let _ = writer.flush().await;
        }
    }

    async fn flush<W>(writer: &mut W, buf: &mut Vec<Frame>)
    where
        W: AsyncWrite + Unpin,
    {
        for frame in buf.drain(..) {
            let _ = write_frame(writer, &frame).await;
        }
        let _ = writer.flush().await;
    }
}
