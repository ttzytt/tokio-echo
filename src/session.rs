use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::common::Config;
use crate::frame::Frame;

pub struct SessionCore {
    pub id: u32,
    pub cfg: Config,
    pub tx_out: UnboundedSender<Frame>,
    pub rx_in: UnboundedReceiver<Vec<u8>>,
}

// sending this to the message handler
pub struct SessionContext {
    id: u32,
    tx_out: UnboundedSender<Frame>,
    rx_in: UnboundedReceiver<Vec<u8>>,
}

impl SessionContext {
    pub fn send(&self, data: Vec<u8>) {
        let _ = self.tx_out.send(Frame { id: self.id, payload: data });
    }

    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.rx_in.recv().await
    }

    pub fn new(id: u32, tx_out: UnboundedSender<Frame>, rx_in: UnboundedReceiver<Vec<u8>>) -> Self {
        SessionContext { id, tx_out, rx_in }
    }
}

impl From<SessionCore> for SessionContext {
    fn from(core: SessionCore) -> Self {
        SessionContext {
            id: core.id,
            tx_out: core.tx_out,
            rx_in: core.rx_in,
        }
    }
}

impl SessionCore {
    pub fn new(id: u32, cfg: Config)
        -> (Self, UnboundedReceiver<Frame>, UnboundedSender<Vec<u8>>)
    {
        let (tx_out, rx_out) = tokio::sync::mpsc::unbounded_channel();
        let (tx_in,  rx_in ) = tokio::sync::mpsc::unbounded_channel();
        (
            SessionCore { id, cfg, tx_out, rx_in },
            rx_out,
            tx_in,
        )
    }
}
