use crate::utils::OneTimeSignal;
use crate::{
    common::{BatchConfig, BoxError, RxOut_t},
    frame::{Frame, write_frame, write_frames},
};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::info;

pub async fn writer_task<W>(
    mut writer: W,
    mut rx_out: RxOut_t, // Receiver for frames to send to peer
    batch: Option<BatchConfig>,
    stop_sig: Option<Arc<OneTimeSignal>>,
) where
    W: AsyncWriteExt + Unpin,
{
    let stop_sig = stop_sig.unwrap_or_else(|| Arc::new(OneTimeSignal::new()));

    if let Some(BatchConfig { size_byte, delay }) = batch {
        let mut buf = Vec::new();
        let mut last = tokio::time::Instant::now();
        let mut buf_bytelen = 0usize;
        loop {
            tokio::select! {
                biased;
                _ = stop_sig.wait() => {
                    buf.clear();
                    let _ = write_frame(&mut writer, &Frame::TERMINATE_ALL_FRAME).await;
                    let _ = writer.flush().await;
                    info!("writer task stopped by signal");
                    break;
                },
                _ = tokio::time::sleep_until(last + delay) => {
                    if !buf.is_empty() {
                        let _ = flush(&mut writer, &mut buf).await;
                        buf.clear(); buf_bytelen = 0;
                    }
                    last = tokio::time::Instant::now();
                },
                frame = rx_out.recv() => {
                    match frame {
                        Some(frame) => {
                              buf_bytelen += Frame::HEADER_BYTES + frame.payload.len();
                            buf.push(frame);
                            if buf_bytelen >= size_byte {
                                let _ = flush(&mut writer, &mut buf).await;
                                buf.clear(); buf_bytelen = 0; last = tokio::time::Instant::now();
                            }
                        },
                        None => break,
                    }
                },
            }
        }
    } else {
        loop {
            tokio::select! {
                biased;
                _ = stop_sig.wait() => {
                    let _ = write_frame(&mut writer, &Frame::TERMINATE_ALL_FRAME).await;
                    let _ = writer.flush().await;
                    break;
                },
                frame = rx_out.recv() => {
                    match frame {
                        Some(frame) => {
                            // info!("writing frame {:?}", frame);
                            // info!("frame payload {}", String::from_utf8_lossy(&frame.payload));
                            write_frame(&mut writer, &frame).await.unwrap();
                            writer.flush().await.unwrap();
                        },
                        None => break,
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
        buf.clear();
        Ok(())
    }
}
