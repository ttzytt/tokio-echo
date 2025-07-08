use crate::{
    bench::{
        comm::{BenchEvent, BenchEventCommunicator},
        config::{BenchConfig, BenchResult, BenchStat},
    },
    client::Client,
    common::{Amrc, BatchConfig, BoxError, ClientHandler, ServerHandler, TransportConfig},
    server::Server,
    session::{ClientSession, ServerSession},
};
use once_cell::sync::OnceCell;
use std::{sync::Arc, time::Duration};
use tokio::{pin, time::Instant};

struct BenchEchoServer {
    pub cfg: BenchConfig,
    pub msg_cnt: OnceCell<usize>,
    pub msg_bytes: OnceCell<usize>,
}

#[async_trait::async_trait]
impl ServerHandler for BenchEchoServer {
    async fn run(&self, sess: Amrc<dyn ServerSession + Send>) {
        println!("server handler start running");
        let mut msg_cnt = 0;
        let mut msg_bytes = 0;
        loop {
            // first wait until we have enough clients
            let sess = sess.lock().await;
            if sess.client_count() < self.cfg.client_cnt {
                println!("current client cnt: {}", sess.client_count());
                tokio::time::sleep(Duration::from_millis(50)).await;
            } else {
                break;
            }
        }
        println!("client reached expected cnt");
        let timer = tokio::time::sleep(self.cfg.last_time);
        pin!(timer);
        loop {
            tokio::select! {
                biased;
                _ = &mut timer => {
                    println!("server timer reached");
                    break;
                }
                mut sess = sess.lock() => {
                    let attempt = sess.recv().await;
                    if let Some((id, payload)) = attempt {
                        msg_cnt += 1;
                        msg_bytes += payload.len();
                        sess.send_to(id, payload);
                    }
                }
            }
        }
        self.msg_cnt.set(msg_cnt).unwrap();
        self.msg_bytes.set(msg_bytes).unwrap();
    }
}

pub struct BenchServerManager {
    pub cases: Vec<BenchConfig>,
    pub comm_addr: String,
    pub srv_addr: String,
    pub comm: BenchEventCommunicator,
}

impl BenchServerManager {
    pub fn new(cases: Vec<BenchConfig>, comm_addr: &str, serv_addr: &str) -> Self {
        Self {
            cases,
            comm_addr: comm_addr.into(),
            comm: BenchEventCommunicator::as_server(comm_addr).unwrap(),
            srv_addr: serv_addr.into(),
        }
    }

    pub fn run_case(&mut self, case: &BenchConfig) -> Result<BenchResult, BoxError> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(case.server_thread_cnt)
            .enable_all()
            .build()?;

        rt.block_on(async {
            let mut msg_tputs: Vec<f64> = Vec::new();
            let mut bytes_tputs: Vec<f64> = Vec::new();
            self.comm
                .send_event(&BenchEvent::ServerSendCaseConfig(case.clone()))?;
            for _ in 0..case.repeat_cnt {
                println!("Running case: {}", case.note);
            
                println!("sent config to clients, waiting for them to connect");
                
                let srv_handler = Arc::new(BenchEchoServer {
                    cfg: case.clone(),
                    msg_cnt: OnceCell::new(),
                    msg_bytes: OnceCell::new(),
                });
                let srv = Arc::from(Server::new(
                    case.tcfg.clone(),
                    srv_handler.clone(),
                    self.srv_addr.as_str(),
                    Some(case.client_cnt),
                ));
                let srv_for_spawn = srv.clone();
                let srv_handle = tokio::spawn(async move {
                    srv_for_spawn.run().await;
                });
                self.comm.send_event(&BenchEvent::ServerSpawned)?;

                self.comm.expect_event(BenchEvent::ClientSpawned)?;
                self.comm
                    .expect_event(BenchEvent::AllClientHandlersConnected)?;

                srv.wait_max_client().await;
                srv.stop_accept();

                srv_handle.await.unwrap();
                self.comm.send_event(&BenchEvent::ServerDone)?;
                self.comm.expect_event(BenchEvent::ClientDone)?;

                let msg_cnt = srv_handler.msg_cnt.get().unwrap();
                let msg_bytes = srv_handler.msg_bytes.get().unwrap();
                let msg_tput = *msg_cnt as f64 / case.last_time.as_secs_f64();
                let bytes_tput = *msg_bytes as f64 / case.last_time.as_secs_f64();
                msg_tputs.push(msg_tput);
                bytes_tputs.push(bytes_tput);
                println!(
                    "Case: {}, Messages: {}, Bytes: {}, Msg Tput: {:.2} msg/s, Bytes Tput: {:.2} KB/s",
                    case.note, msg_cnt, msg_bytes, msg_tput, bytes_tput / 1000.0
                );
            }
            let msg_tput_stat = BenchStat::<f64, f64>::new(msg_tputs, true);
            let bytes_tput_stat = BenchStat::<f64, f64>::new(bytes_tputs, true);
            let latency_stat_event = self
                .comm
                .expect_event(BenchEvent::ClientSendLatencyStat(
                    BenchStat::<f64, usize>::dummy(),
                ))?;
            let latency_stat = match latency_stat_event {
                BenchEvent::ClientSendLatencyStat(stat) => stat,
                _ => return Err("Expected ClientSendLatencyStat event".into()),
            };
                
            Ok(BenchResult {
                latency_ms: latency_stat,
                throughput_msg: msg_tput_stat,
                throughput_bytes: bytes_tput_stat,
                note: String::new(),
            })
        })
    }

    pub fn run(&mut self) -> Result<Vec<BenchResult>, BoxError> {
        let case_cnt = self.cases.len() as u32;
        self.comm.send_event(&BenchEvent::ServerSendCaseCnt(case_cnt))?;
        let mut results: Vec<BenchResult> = Vec::new();
        for case in self.cases.clone() {
            let result = self.run_case(&case)?;
            results.push(result);
        }
        Ok(results)
    }
}
