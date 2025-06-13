use crate::{
    common::{BatchConfig, BoxError, RxOut},
    frame::{Frame, write_frame, write_frames},
};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;
use std::sync::Arc;

pub async fn writer_task<W>(
    mut writer: W,
    mut rx_out: RxOut,
    batch: Option<BatchConfig>,
    stop_sig : Option<Arc<Notify>>,
)
where
    W: AsyncWriteExt + Unpin,
{
    let stop_sig = stop_sig.unwrap_or_else(|| Arc::new(Notify::new()));

    if let Some(BatchConfig { size_byte, delay }) = batch {
        let mut buf = Vec::new();
        let mut last = tokio::time::Instant::now();
        let mut buf_bytelen = 0usize;
        loop {
            tokio::select! {
                biased;
                _ = stop_sig.notified() => {
                    if !buf.is_empty() {
                        flush(&mut writer, &mut buf).await.unwrap();
                    }
                    break;
                },
                _ = tokio::time::sleep_until(last + delay) => {
                    if !buf.is_empty() { 
                        flush(&mut writer, &mut buf).await.unwrap();
                    }
                    last = tokio::time::Instant::now();
                },
                frame = rx_out.recv() => {
                    match frame {
                        Some(frame) => {
                            buf_bytelen += 8 + frame.payload.len(); // 8 bytes for id and length
                            buf.push(frame);
                            if buf_bytelen >= size_byte {
                                flush(&mut writer, &mut buf).await.unwrap();
                                last = tokio::time::Instant::now();
                            }
                        },
                        None => break,
                    }
                },
            }
        }

    } else {
        loop{
            tokio::select! {
                biased;
                _ = stop_sig.notified() => {
                    break;
                },
                frame = rx_out.recv() => {
                    match frame {
                        Some(frame) => {
                            write_frame(&mut writer, &frame).await.unwrap();
                            writer.flush().await.unwrap();
                        },
                        None => break, // Channel closed
                    }
                },
            }
        }
    }

    async fn flush<W>(writer: &mut W, buf: &mut Vec<Frame>) -> Result<(), BoxError>
    where
        W: AsyncWriteExt + Unpin,
    {   
        write_frames(writer, buf).await?;
        writer.flush().await?;
        Ok(())
    }
}
