use crate::frame::{Frame, read_frame};
use tokio::io::AsyncRead;
use tokio::sync::mpsc::{UnboundedSender};

use crate::common::{BoxError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;

pub enum ReaderTxInOpt{
    TxIn(UnboundedSender<Frame>), 
    IdToTxIn(HashMap<u32, UnboundedSender<Frame>>),
}


pub async fn frame_reader_task<R>(
    mut reader: R,
    tx_in_opt: ReaderTxInOpt,
    stop_sig: Option<Arc<Notify>>,
) -> Result<(), BoxError>
where
    R: AsyncRead + Unpin,
{
    let stop_sig = stop_sig.unwrap_or_else(|| Arc::new(Notify::new()));
    loop {
        tokio::select! {
            biased;
            _ = stop_sig.notified() => {
                break;
            },
            frame = read_frame(&mut reader) => {
                match frame {
                    Ok(frame) => {
                        match &tx_in_opt{
                            ReaderTxInOpt::TxIn(tx) => {
                                _ = tx.send(frame)
                            },
                            ReaderTxInOpt::IdToTxIn(map) => {
                                if let Some(tx) = map.get(&frame.id) {
                                    let _ = tx.send(frame);
                                } 
                            }
                        };
                    }
                    Err(e) => {
                        tracing::error!("Error reading frame: {}", e);
                        return Err(e);
                    }
                }
            },
        }
    }
    Ok(())
}
