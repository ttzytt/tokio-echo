use crate::{
    common::BatchConfig,
    frame::{Frame, write_frame},
};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedReceiver;

pub async fn writer_task<W>(
    mut writer: W,
    mut rx: UnboundedReceiver<Frame>,
    batch: Option<BatchConfig>,
) where
    W: AsyncWriteExt + Unpin,
{
    if let Some(BatchConfig { size, delay }) = batch {
        let mut buf = Vec::with_capacity(size);
        let mut last = tokio::time::Instant::now();
        while let Some(frame) = rx.recv().await {
            buf.push(frame);
            if buf.len() >= size {
                flush(&mut writer, &mut buf).await;
                last = tokio::time::Instant::now();
            }
            tokio::select! {
                _ = tokio::time::sleep_until(last + delay) => {
                    if !buf.is_empty(){ 
                        flush(&mut writer, &mut buf).await;
                        last = tokio::time::Instant::now();
                    }
                }
                else => {}
            }
        }
    } else {
        while let Some(frame) = rx.recv().await {
            let _ = write_frame(&mut writer, &frame).await;
            let _ = writer.flush().await;
        }
    }

    async fn flush<W>(writer: &mut W, buf: &mut Vec<Frame>)
    where
        W: AsyncWriteExt + Unpin,
    {   
        for frame in buf.drain(..) {
            let _ = write_frame(writer, &frame).await;
        }
        let _ = writer.flush().await;
    }
}
