use crate::{
    common::{TransportConfig, BatchConfig}
};

use std::time::Duration;
use serde::{Serialize, Deserialize};
use statrs::statistics::Statistics;
use num_traits::{Num, NumCast, ToPrimitive};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BenchConfig {
    pub tcfg : TransportConfig, 
    pub client_cnt : usize, 
    pub repeat_cnt: usize,
    pub server_thread_cnt: usize,
    pub client_thread_cnt: usize,
    pub last_time: Duration, 
    pub addl_payload_bytes: usize, // payloads are random
    pub note : String,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self{
            tcfg: TransportConfig::DEFAULT,
            client_cnt: 0,
            repeat_cnt: 0,
            server_thread_cnt: 0,
            client_thread_cnt: 0,
            last_time: Duration::from_secs(0),
            addl_payload_bytes: 0,
            note: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BenchStat<T, O>{
    pub avg: T, 
    pub sdev: T,
    pub raw: Option<Vec<O>>,
}

impl<T, O> BenchStat<T, O>
where
    T: Num + NumCast + Copy,
    O: ToPrimitive + Clone,
{
    pub fn new<I>(data: I, keep_raw: bool) -> Self
    where
        I: IntoIterator<Item = O>,
    {
        let raw_vec: Vec<O> = data.into_iter().collect();
        let values_f64: Vec<f64> = raw_vec
            .iter()
            .filter_map(|x| x.to_f64())
            .collect();

        let slice = values_f64.as_slice();
        let mean_f64 = slice.mean();
        let sdev_f64 = slice.std_dev();
            
        BenchStat {
            avg: T::from(mean_f64).unwrap(),
            sdev: T::from(sdev_f64).unwrap(),
            raw: if keep_raw { Some(raw_vec) } else { None },
        }
    }

    pub fn dummy() -> Self {
        BenchStat {
            avg: T::zero(),
            sdev: T::zero(),
            raw: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BenchResult {
    pub latency_ms: BenchStat<f64, usize>, // in milliseconds
    pub throughput_msg: BenchStat<f64, f64>, // messages per second  
    pub throughput_bytes: BenchStat<f64, f64>, // bytes per second
    pub note: String,
}


#[test]
fn test_serde(){
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
    let serialized = serde_json::to_string(&cfg).unwrap();
    println!("Serialized: {}", serialized);
}