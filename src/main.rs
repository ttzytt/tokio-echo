mod server;
mod frame;
mod common;
mod session;
mod session_starter;
mod transport;
mod client;
use std::sync::Arc;
use crate::{
    common::{Config, ClientHandler, ServerHandler, BoxError},
    client::Client,
    server::Server,
    session::{ClientSession, ServerSession},
};

struct EchoServer;
#[async_trait::async_trait]
impl ServerHandler for EchoServer {
    async fn run(&self, sess: &mut ServerSession) {
        println!("EchoServer started");
        while let Some((id, msg)) = sess.recv().await {
            let text = String::from_utf8_lossy(&msg);
            let ack = format!("ack {}: {}", id, text).into_bytes();
            sess.send_to(id, ack);
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
            // tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let cfg = Config { threads: 4, use_mux: true, batch: None };

    // Start server
    let srv = Server::new(cfg.clone(), Arc::new(EchoServer), "localhost:8080");
    tokio::spawn(async move { srv.run().await.unwrap() });

    // Start client
    let mut cli = Client::new(cfg, "localhost:8080");
    cli.register(Arc::new(HellosClient));
    cli.register(Arc::new(HellosClient)); // Register multiple handlers if needed
    cli.run().await?;
    println!("Client finished running");
    // make it never return 
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    Ok(())

}