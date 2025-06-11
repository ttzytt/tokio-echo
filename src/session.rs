use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::common::Config;
use crate::frame::Frame;

/// RawSession exposes Frame‐level send/recv.
/// 
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
    id: u32,
}

impl ClientSession {
    pub fn new(raw: RawSession, id: u32) -> Self {
        Self { raw, id }
    }

    pub fn send(&self, payload: Vec<u8>) {
        let _ = self.raw.tx_out.send(Frame { id: self.id, payload });
    }

    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.raw.rx_in.recv().await.map(|f| f.payload)
    }
}


#[async_trait::async_trait]
pub trait ServerSession{
    async fn recv(&mut self) -> Option<(u32, Vec<u8>)>;
    fn send_to(&self, sub_id: u32, payload: Vec<u8>);

}
