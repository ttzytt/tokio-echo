use crate::frame::{Frame, read_frame};
use tokio::io::AsyncRead;
use tokio::sync::mpsc::UnboundedSender;

use crate::common::{BoxError, Id_t};
use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::sync::Arc;
use crate::utils::OneTimeSignal;

pub enum ReaderTxInOpt {
    TxIn(UnboundedSender<Frame>),
    IdToTxIn(HashMap<Id_t, UnboundedSender<Frame>>),
}

pub async fn frame_reader_task<R, F>(
    mut reader: R,
    tx_in_opt: ReaderTxInOpt,
    stop_sig: Option<Arc<OneTimeSignal>>,
    on_frame: Option<F>,
) -> Result<(), BoxError>
where
    R: AsyncRead + Unpin,
    F: Fn(&Frame),
{
    let stop_sig = stop_sig.unwrap_or_else(|| Arc::new(OneTimeSignal::new()));
    let mut prev_frame = Frame::TERMINATE_ALL_FRAME;
    loop {
        tokio::select! {
            biased;
            _ = stop_sig.wait() => {
                break;
            },
            frame = read_frame(&mut reader) => {
                match frame {
                    Ok(frame) => {
                        match &tx_in_opt{
                            ReaderTxInOpt::TxIn(tx) => {
                                if let Some(cb) = on_frame.as_ref() {
                                    // cb = callback
                                    cb(&frame);
                                }
                                prev_frame = frame.clone();
                                _ = tx.send(frame)
                            },
                            ReaderTxInOpt::IdToTxIn(map) => {
                                if let Some(tx) = map.get(&frame.id) {
                                    prev_frame = frame.clone();
                                    let _ = tx.send(frame);
                                }
                            }
                        };
                    }
                    Err(e) => {
                        tracing::error!("Error reading frame: {}\n
                                         prev frame: {:?}\n
                                         Backtrace: {}", 
                                         e, prev_frame, Backtrace::capture());
                        return Err(e);
                    }
                }
            },
        }
    }
    Ok(())
}
