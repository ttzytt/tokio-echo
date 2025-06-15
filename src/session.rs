// src/session.rs
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use crate::common::{Id_t, ByteSeq};
use crate::frame::{Frame, FrameKind};

pub struct RawSession {
    pub tx_out: UnboundedSender<Frame>,
    pub rx_in: UnboundedReceiver<Frame>,
}

impl RawSession {
    /// Creates channels: (RawSession, rx_to_writer, tx_from_reader)
    pub fn new() -> (Self, UnboundedReceiver<Frame>, UnboundedSender<Frame>) {
        let (tx_out, rx_out) = tokio::sync::mpsc::unbounded_channel();
        let (tx_in, rx_in) = tokio::sync::mpsc::unbounded_channel();
        (RawSession { tx_out, rx_in }, rx_out, tx_in)
    }
}

/// ClientSession hides stream IDs: send/recv only payloads.
pub struct ClientSession {
    raw: RawSession,
    id: Id_t,
}

impl ClientSession {
    pub fn new(raw: RawSession, id: Id_t) -> Self {
        Self { raw, id }
    }

    pub fn send(&self, payload: ByteSeq) {
        let _ = self.raw.tx_out.send(Frame {
            kind: FrameKind::Data,
            id: self.id,
            payload,
        });
    }

    pub async fn recv(&mut self) -> Option<ByteSeq> {
        self.raw.rx_in.recv().await.map(|f| f.payload)
    }
}

#[async_trait::async_trait]
pub trait ServerSession{
    async fn recv(&mut self) -> Option<(Id_t, ByteSeq)>;
    fn send_to(&self, sub_id: Id_t, payload: ByteSeq);
    fn is_connected(&self, sub_id: Id_t) -> bool;
    fn send_if_connected(&self, sub_id: Id_t, payload: ByteSeq) -> bool {
        if self.is_connected(sub_id) {
            self.send_to(sub_id, payload);
            true
        } else {
            false
        }
    }
    fn client_count(&self) -> usize;
}
