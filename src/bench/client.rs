use crate::{
    bench::{
        comm::{BenchEvent, BenchEventCommunicator},
        config::{BenchConfig, BenchResult, BenchStat},
    },
    client::Client,
    common::{Amrc, BatchConfig, BoxError, ClientHandler, TransportConfig},
    server::Server,
    session::{ClientSession, ServerSession},
    utils::OneTimeSignal,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::thread;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpStream, pin, sync::Notify, time::Instant};

#[derive(Serialize, Deserialize, Debug)]
struct EchoFrame {
    pub fid: u32,
    pub rand_bytes: Vec<u8>,
}

impl EchoFrame {
    pub fn new(fid: u32, rand_bytes_len: usize) -> Self {
        let ascii_a = b'a';
        Self {
            fid,
            rand_bytes: vec![ascii_a; rand_bytes_len],
        }
    }
}

struct BenchEchoServer {
    pub cfg: BenchConfig,
    pub latencies: OnceCell<Vec<u16>>,
    pub all_handler_connected_sig: Arc<OneTimeSignal>,
}

#[async_trait::async_trait]
impl ClientHandler for BenchEchoServer {
    async fn run(&self, sess: &mut ClientSession) {
        self.all_handler_connected_sig.wait().await;
        let timer = tokio::time::sleep(self.cfg.last_time);
        pin!(timer);
        let mut msg_cnt = 0;
        let mut fid_to_sent_time: HashMap<u32, Instant> = HashMap::new();
        let mut latencies: Vec<u16> = Vec::new();
        loop {
            let send_msg = EchoFrame::new(msg_cnt as u32, self.cfg.addl_payload_bytes);
            sess.send(serde_json::to_vec(&send_msg).unwrap());
            fid_to_sent_time.insert(msg_cnt as u32, Instant::now());
            msg_cnt += 1;
            tokio::select! {
                biased;
                _ = &mut timer => {
                    println!("client timer reached");
                    break;
                },
                recv_msg = sess.recv() => {
                    if let Some(recv_msg) = recv_msg {
                        let echoed_frame =  serde_json::from_slice::<EchoFrame>(&recv_msg).unwrap();
                        if let Some(sent_time) = fid_to_sent_time.remove(&echoed_frame.fid) {
                            let latency = sent_time.elapsed().as_millis() as u16;
                            latencies.push(latency);
                        } else {
                            eprintln!("Received frame with fid {} that was not sent", echoed_frame.fid);
                        }
                    }
                }
            }
        }
        self.latencies.set(latencies).unwrap();
    }
}
pub struct BenchClientManager {
    pub comm_srv_addr: String,
    pub srv_addr: String,
    pub comm: BenchEventCommunicator,
}

impl BenchClientManager {
    pub fn new(comm_srv_addr: &str, serv_addr: &str) -> Self {
        Self {
            comm_srv_addr: comm_srv_addr.into(),
            comm: BenchEventCommunicator::as_client(comm_srv_addr).unwrap(),
            srv_addr: serv_addr.into(),
        }
    }

    pub fn run_case(&mut self) -> Result<(), BoxError> {
        println!("Waiting for ServerSendConfig event...");
        let cfg_event = self
            .comm
            .expect_event(BenchEvent::ServerSendCaseConfig(BenchConfig::default()))?;

        let cfg = match cfg_event {
            BenchEvent::ServerSendCaseConfig(cfg) => cfg,
            _ => return Err("Expected ServerSendConfig event".into()),
        };

        println!("received config: {:?}", cfg);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(cfg.client_thread_cnt)
            .enable_all()
            .build()?;

        rt.block_on(async {
            let mut latencies: Vec<u16> = Vec::new();
            for _ in 0..cfg.repeat_cnt {
                let mut client = Client::new(cfg.tcfg.clone(), self.srv_addr.as_str());
                let mut handlers: Vec<Arc<BenchEchoServer>> = Vec::new();
                for _ in 0..cfg.client_cnt {
                    let handler = Arc::new(BenchEchoServer {
                        cfg: cfg.clone(),
                        latencies: OnceCell::new(),
                        all_handler_connected_sig: client.all_handler_connected_sig.clone(),
                    });
                    handlers.push(handler.clone());
                    client.register(handler);
                }
                println!("client created {} handlers", handlers.len());

                let all_handler_connected_sig = client.all_handler_connected_sig.clone();
                self.comm.expect_event(BenchEvent::ServerSpawned)?;
                // wait for server to accept 
                tokio::time::sleep(Duration::from_millis(100)).await;
                let ch_join_handle = tokio::spawn(async move { client.run().await });

                self.comm.send_event(&BenchEvent::ClientSpawned)?;

                all_handler_connected_sig.wait().await;
                self.comm
                    .send_event(&BenchEvent::AllClientHandlersConnected)?;

                ch_join_handle.await.unwrap();
                self.comm.send_event(&BenchEvent::ClientDone)?;
                self.comm.expect_event(BenchEvent::ServerDone)?;

                for h in handlers {
                    latencies.extend(h.latencies.get().unwrap())
                }
            }
            let latencies: Vec<usize> = latencies.into_iter().map(|x| x as usize).collect();
            let latency_stat = BenchStat::<f64, usize>::new(latencies, false);
            self.comm
                .send_event(&BenchEvent::ClientSendLatencyStat(latency_stat))?;
            Ok::<(), BoxError>(())
        })?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), BoxError> {
        let case_cnt_event = self.comm.expect_event(BenchEvent::ServerSendCaseCnt(0))?;
        let case_cnt = match case_cnt_event  {
            BenchEvent::ServerSendCaseCnt(cnt) => cnt,
            _ => return Err("Expect receive ServerSendCaseCnt event".into()),
        };
        for _ in 0..case_cnt{
            self.run_case()?;
        }
        Ok(())
    }
}
