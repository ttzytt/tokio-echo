use crate::{
    common::{BoxError, Config, ServerHandler, Amrc},
    frame::Frame,
    session::{RawSession, ServerSession},
    transport::{
        readers::{frame_reader_task},
        writers::writer_task,
    },
};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::io::split;

pub struct MuxServerSession {
    raw: RawSession,
}

#[async_trait::async_trait]
impl ServerSession for MuxServerSession {
    async fn recv(&mut self) -> Option<(u32, Vec<u8>)> {
        self.raw.rx_in.recv().await.map(|f| (f.id, f.payload))
    }

    fn send_to(&self, sub_id: u32, payload: Vec<u8>) {
        let _ = self.raw.tx_out.send(Frame {
            id: sub_id,
            payload,
        });
    }
}

impl MuxServerSession {
    pub fn new(raw: RawSession) -> Self {
        Self { raw }
    }
}

pub struct SimpleServerSession {
    tcp_sessions: Vec<RawSession>,
}

#[async_trait::async_trait]
impl ServerSession for SimpleServerSession {
    async fn recv(&mut self) -> Option<(u32, Vec<u8>)> {
        for (i, session) in self.tcp_sessions.iter_mut().enumerate() {
            if let Ok(frame) = session.rx_in.try_recv() {
                return Some(((i + 1) as u32, frame.payload));
            }
        }
        None
    }

    fn send_to(&self, id: u32, payload: Vec<u8>) {
        let single_sess = &self.tcp_sessions[id as usize - 1];
        let _ = single_sess.tx_out.send(Frame { id: 0, payload });
    }
}

impl SimpleServerSession {
    pub fn reg_new_session(&mut self, sock: TcpStream, id: u32, cfg: Config) {
        let (r, w) = split(sock);
        let (raw, rx_out, tx_in) = RawSession::new();

        tokio::spawn(crate::transport::writers::writer_task(
            w,
            rx_out,
            cfg.batch.clone(),
        ));
        tokio::spawn(crate::transport::readers::frame_reader_task(
            r,
            tx_in.clone(),
        ));

        self.tcp_sessions.push(raw);
        assert!(self.tcp_sessions.len() == id as usize);
    }

    pub fn new() -> Self {
        Self {
            tcp_sessions: Vec::new(),
        }
    }
}

pub struct Server {
    cfg: Config,
    handler: Arc<dyn ServerHandler>,
    addr: String,
}

impl Server {
    pub fn new(cfg: Config, handler: Arc<dyn ServerHandler>, addr: &str) -> Self {
        Self {
            cfg,
            handler,
            addr: addr.into(),
        }
    }

    pub async fn run(&self) -> Result<(), BoxError> {
        let listener = TcpListener::bind(&self.addr).await?;
        if self.cfg.use_mux {
            let (sock, _) = listener.accept().await?;
            Server::start_mux_server(sock, self.handler.clone(), self.cfg.clone());
        } else {
            let mut next_id = 1;
            let simple_session = Amrc::new(Mutex::new(SimpleServerSession::new()));
            let handler = self.handler.clone();
            let sess_for_spawn = simple_session.clone();
            let cfg = self.cfg.clone();

            tokio::spawn(async move {
                handler.run(sess_for_spawn).await;
            });

            loop {
                let (sock, _) = listener.accept().await.unwrap();
                let mut sess = simple_session.lock().await;
                sess.reg_new_session(sock, next_id, cfg.clone());
                next_id += 1;
            };
        
        }
        Ok(())
    }

    fn start_mux_server(socket: TcpStream, handler: Arc<dyn ServerHandler>, cfg: Config) {
        let (r, w) = split(socket);
        let (raw_sess, rx_out, tx_in) = RawSession::new();
    
        // 1) spawn the write loop
        tokio::spawn(writer_task(w, rx_out, cfg.batch.clone()));
        tokio::spawn(frame_reader_task(r, tx_in));
        tokio::spawn(async move {
            let serv_sess = MuxServerSession::new(raw_sess);
            handler.run(Amrc::from(Mutex::from(serv_sess))).await;
        });
    }

}
