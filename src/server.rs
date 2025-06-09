use futures::future::BoxFuture;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    runtime::Builder,
    time::{Instant, sleep_until},
};

use crate::common::{BoxError, Handler, Config};
use crate::frame::{Frame, read_frame, write_frame};

use once_cell::sync::Lazy;

static ECHO_HANDLER: Lazy<Handler> = Lazy::new(|| {
    Arc::new(|_id, data| {
        Box::pin(async move { data })
    })
});

pub struct Server {
    pub cfg: Config,
    pub handler: Handler,
    pub conn_cnt: AtomicU32,
}

impl Server {
    pub fn new(cfg: Config, handler: Handler) -> Self {
        Self {
            cfg,
            handler,
            conn_cnt: AtomicU32::new(1),
        }
    }

    pub fn run(self, addr: &str) -> Result<(), BoxError> {
        let rt = Builder::new_multi_thread()
            .worker_threads(self.cfg.threads)
            .enable_all()
            .build()?;
        let server = Arc::new(self);

        rt.block_on(async move {
            let listener = TcpListener::bind(addr).await.unwrap();
            println!("Listening on {}", addr);

            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let server = server.clone();
                tokio::spawn(async move {
                    let stream_id = server.conn_cnt.fetch_add(1, Ordering::SeqCst);
                    server
                        .handle_conn(socket, stream_id)
                        .await
                        .unwrap_or_else(|e| {
                            eprintln!("[id={}] error: {}", stream_id, e);
                        });
                });
            }
        });

        Ok(())
    }

    async fn handle_conn(
        self: Arc<Self>,
        socket: TcpStream,
        stream_id: u32,
    ) -> Result<(), BoxError> {
        socket.set_nodelay(self.cfg.batching)?;
        match (self.cfg.batching, self.cfg.use_mux) {
            (true, _) => self.handle_with_batch(socket, stream_id).await,
            (false, true) => self.handle_mux(socket).await,
            (false, false) => self.handle_simple(socket, stream_id).await,
        }
    }

    async fn handle_simple(&self, socket: TcpStream, stream_id: u32) -> Result<(), BoxError> {
        let (r, mut w) = socket.into_split();
        let mut reader = BufReader::new(r).lines();

        while let Some(line) = reader.next_line().await? {
            let data = line.into_bytes();
            let resp = (self.handler)(stream_id, data).await;
            w.write_all(&resp).await?;
            w.write_all(b"\n").await?;
            w.flush().await?;
        }
        Ok(())
    }

    async fn handle_mux(&self, socket: TcpStream) -> Result<(), BoxError> {
        let (mut r, mut w) = socket.into_split();
        loop {
            let frame = read_frame(&mut r).await?;
            let resp = (self.handler)(frame.id, frame.payload).await;
            write_frame(
                &mut w,
                &Frame {
                    id: frame.id,
                    payload: resp,
                },
            )
            .await?;
        }
    }

    async fn handle_with_batch(&self, socket: TcpStream, stream_id: u32) -> Result<(), BoxError> {
        let (mut r, mut w) = socket.into_split();
        let mut batch: Vec<Frame> = Vec::with_capacity(self.cfg.batch_size);
        let mut last = Instant::now();

        loop {
            tokio::select! {
                frame = async {
                    if self.cfg.use_mux {
                        read_frame(&mut r).await
                    } else {
                        let mut line = String::new();
                        let n = bufreader_next_line(&mut r, &mut line).await?;
                        if n == 0 {
                            return Err("EOF".into());
                        }
                        Ok(Frame { id: stream_id, payload: line.into_bytes() })
                    }
                } => {
                    let frame = frame?;
                    let resp_payload = (self.handler)(frame.id, frame.payload).await;
                    batch.push(Frame{id:frame.id, payload: resp_payload});
                    if batch.len() >= self.cfg.batch_size {
                        self.flush_batch(&mut w, &mut batch).await?;
                        last = Instant::now();
                    }
                }

                _ = sleep_until(last + Duration::from_millis(self.cfg.batch_delay_ms)) => {
                    if !batch.is_empty() {
                        self.flush_batch(&mut w, &mut batch).await?;
                        last = Instant::now();
                    }
                }
            }
        }
    }

    async fn flush_batch<W>(&self, writer: &mut W, batch: &mut Vec<Frame>) -> Result<(), BoxError>
    where
        W: AsyncWriteExt + Unpin,
    {
        for frame in batch.drain(..) {
            if self.cfg.use_mux {
                write_frame(
                    writer,
                    &frame
                )
                .await?;
            } else {
                writer.write_all(&frame.payload).await?;
                writer.write_all(b"\n").await?;
            }
        }
        writer.flush().await?;
        Ok(())
    }
}

async fn bufreader_next_line<R>(reader: &mut R, buf: &mut String) -> Result<usize, BoxError>
where
    R: AsyncReadExt + Unpin,
{
    let mut temp = BufReader::new(reader);
    let n = temp.read_line(buf).await?;
    Ok(n)
}
