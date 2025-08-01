use std::{env, sync::Arc, time::Duration};
use tokio::pin;
use tokio_echo::{
    client::Client,
    common::{Amrc, BatchConfig, BoxError, ClientHandler, TransportConfig, ServerHandler},
    server::Server,
    session::{ClientSession, ServerSession},
};

struct EchoServer;
#[async_trait::async_trait]
impl ServerHandler for EchoServer {
    async fn run(&self, sess: Amrc<dyn ServerSession + Send>) {
        println!("EchoServer started");
        const LAST_TIME_S: u64 = 2;
        let timer = tokio::time::sleep(Duration::from_secs(LAST_TIME_S));
        pin!(timer);

        loop {
            tokio::select! {
                biased;
                _ = &mut timer => {
                    println!("EchoServer: no activity for {} seconds, exiting", LAST_TIME_S);
                    return;
                },
                // otherwise send the message through sess
                mut sess = sess.lock() => {
                    let attempt = sess.recv().await;
                    if let Some((id, payload)) = attempt {
                        println!("server got: id={}, payload={}", id, String::from_utf8_lossy(&payload));
                        let ack = format!("ack: {}: {}", id, String::from_utf8_lossy(&payload)).into_bytes();
                        println!("server sent: {}", String::from_utf8_lossy(&ack));
                        sess.send_to(id, ack);
                    }
                },
            }
        }
    }
}

struct HellosClient;
#[async_trait::async_trait]
impl ClientHandler for HellosClient {
    async fn run(&self, sess: Amrc<ClientSession>) {
        const LAST_TIME_S: u64 = 1;
        let timer = tokio::time::sleep(Duration::from_secs(LAST_TIME_S));
        pin!(timer);
        println!("HellosClient started");
        let mut msg_cnt = 0;
        loop {
            // clone the Arc to keep it alive during the lock
            let sess_arc = sess.clone();
            let mut sess = sess_arc.lock().await;
            let msg = format!("hello world {}", msg_cnt);
            println!("client sent:\n{}", msg);
            msg_cnt += 1;
            sess.send(msg.into_bytes());
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            tokio::select! {
                biased;
                _ = &mut timer => {
                    println!("HellosClient: no activity for {} seconds, exiting", LAST_TIME_S);
                    return;
                },
                msg = sess.recv() => {
                    if let Some(msg) = msg {
                        println!("client got: {}", String::from_utf8_lossy(&msg));
                    }
                },
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt::init();
    unsafe { env::set_var("RUST_BACKTRACE", "full") };
    let cfg = TransportConfig {
        use_mux: false,
        batch: Some(BatchConfig {
            size_byte: 128,
            delay: Duration::from_millis(100),
        }),
    };
    let cfg = TransportConfig { use_mux: false, batch: None};

    let srv = Arc::from(Server::new(
        cfg.clone(),
        Arc::new(EchoServer),
        "localhost:4321",
        None,
    )); 
    let srv_for_spawn = srv.clone();
    let srv_handle = tokio::spawn(async move { srv_for_spawn.run().await });

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let mut cli = Client::new(cfg, "localhost:4321");
    cli.register(Arc::new(HellosClient));
    cli.register(Arc::new(HellosClient));

    cli.run().await;
    println!("Clients finished running");

    srv.stop_accept();
    srv_handle.await.unwrap();
    println!("Server finished running");

    Ok(())
}
