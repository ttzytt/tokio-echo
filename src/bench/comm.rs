use std::net::{TcpListener, TcpStream};
use std::io::{BufReader, Read, Write};
use std::mem::discriminant;
use serde::{Deserialize, Serialize};
use crate::bench::config::{BenchConfig, BenchStat};
use crate::common::{BoxError, TransportConfig};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
pub enum BenchEvent {
    ServerSendCaseCnt(u32),
    ServerSendCaseConfig(BenchConfig),
    ClientSendLatencyStat(BenchStat<f64, usize>),
    ServerSpawned,
    ClientSpawned,
    AllClientHandlersConnected,
    ServerDone,
    ClientDone,
}

pub struct BenchEventCommunicator {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl BenchEventCommunicator {
    /// Block until a client connects, then return communicator
    pub fn as_server(addr: &str) -> Result<Self, BoxError> {
        let listener = TcpListener::bind(addr)?;
        let (stream, _) = listener.accept()?;
        stream.set_nodelay(true)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self { reader, writer: stream })
    }

    /// Connect to server, then return communicator
    pub fn as_client(addr: &str) -> Result<Self, BoxError> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self { reader, writer: stream })
    }

    /// Serialize and send an event (length + payload)
    pub fn send_event(&mut self, event: &BenchEvent) -> Result<(), BoxError> {
        println!("sent event {:?}", event);
        let payload = serde_json::to_vec(event)?;
        let len_bytes = (payload.len() as u32).to_be_bytes();
        self.writer.write_all(&len_bytes)?;
        self.writer.write_all(&payload)?;
        Ok(())
    }

    /// Read exactly one event (length + payload)
    pub fn recv_event(&mut self) -> Result<BenchEvent, BoxError> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        self.reader.read_exact(&mut data)?;
        let ret = serde_json::from_slice(&data)?;
        println!("received event {:?}", ret);
        Ok((ret))
    }

    /// Receive and verify the expected event variant
    pub fn expect_event(&mut self, expected: BenchEvent) -> Result<BenchEvent, BoxError> {
        let actual = self.recv_event()?;
        if discriminant(&actual) != discriminant(&expected) {
            return Err(format!("expected {:?}, got {:?}", expected, actual).into());
        }
        Ok(actual)
    }
}

#[test]
fn test_bench_event_serde() {
    let event = BenchEvent::ClientDone;
    println!("Serialized: {}", serde_json::to_string(&event).unwrap());

    let cfg = BenchConfig {
        tcfg: TransportConfig::DEFAULT,
        client_cnt: 10,
        repeat_cnt: 100,
        server_thread_cnt: 4,
        client_thread_cnt: 4,
        last_time: Duration::from_secs(10),
        addl_payload_bytes: 0,
        note: "test".to_string(),
    };
    let event = BenchEvent::ServerSendCaseConfig(cfg);
    println!("Serialized with config: {}", serde_json::to_string(&event).unwrap());

    let stat = BenchStat::new(vec![1, 2, 3, 4], true);
    let event = BenchEvent::ClientSendLatencyStat(stat);
    let s = serde_json::to_string(&event).unwrap();
    println!("Serialized with latency stat: {}", s);
    let de: BenchEvent = serde_json::from_str(&s).unwrap();
    println!("Deserialized: {:?}", de);
}
