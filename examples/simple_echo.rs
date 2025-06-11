use std::{sync::Arc, time::Duration};
use futures::FutureExt;

use tokio_echo::{
    common::{Config, ClientHandler, ServerHandler, BoxError, BatchConfig, Amrc},
    client::Client,
    server::Server,
    session::{ClientSession, ServerSession},
};

struct EchoServer;
#[async_trait::async_trait]
impl ServerHandler for EchoServer {
    async fn run(&self, sess: Amrc<dyn ServerSession + Send>){
        println!("EchoServer started");
        loop {
            let mut sess = sess.lock().await;
            let attempt = sess.recv().now_or_never();
            if let Some(Some((id, payload))) = attempt {
                println!("server got: id={}, payload={}", id, String::from_utf8_lossy(&payload));
                let ack = format!("ack: {}: {}", id, String::from_utf8_lossy(&payload)).into_bytes();
                sess.send_to(id, ack);
            } 
        }
    }
}

struct HellosClient;
#[async_trait::async_trait]
impl ClientHandler for HellosClient {
    async fn run(&self, sess: &mut ClientSession) {
        println!("HellosClient started");
        loop {
            println!("client sending: hello world");
            sess.send(b"hello world".to_vec());
            if let Some(resp) = sess.recv().await {
                println!("client got: {}", String::from_utf8_lossy(&resp));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let cfg = Config { use_mux: false, batch: Some(BatchConfig { size: 1024, delay: Duration::from_millis(50) }) };

    // Start server
    let srv = Server::new(cfg.clone(), Arc::new(EchoServer), "localhost:4321");
    tokio::spawn(async move { srv.run().await.unwrap();});

    // wait for a second then start client
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let mut cli = Client::new(cfg, "localhost:4321");
    cli.register(Arc::new(HellosClient));
    cli.register(Arc::new(HellosClient)); // Register multiple handlers if needed
    cli.run().await?;
    println!("Client finished running");
    // make it never return 
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    Ok(())

}