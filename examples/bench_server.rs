use std::env;
use std::time::Duration;
use tokio;
use tokio_echo::{
    bench::{config::BenchConfig, server::BenchServerManager},
    common::{BatchConfig, TransportConfig},
};

fn main() {
    unsafe { env::set_var("RUST_BACKTRACE", "full") };
    tracing_subscriber::fmt::init();

    let cases = vec![
        // BenchConfig {
        //     tcfg: TransportConfig {
        //         use_mux: false,
        //         batch: None,
        //     },
        //     client_cnt: 15,
        //     repeat_cnt: 5,
        //     server_thread_cnt: 1,
        //     client_thread_cnt: 15,
        //     last_time: std::time::Duration::from_secs(3),
        //     addl_payload_bytes: 0,
        //     note: "Test case 1".to_string(),
        // },
        // BenchConfig {
        //     tcfg: TransportConfig {
        //         use_mux: true,
        //         batch: None,
        //     },
        //     client_cnt: 10,
        //     repeat_cnt: 3 ,
        //     server_thread_cnt: 1,
        //     client_thread_cnt: 10,
        //     last_time: std::time::Duration::from_secs(3),
        //     addl_payload_bytes: 0,
        //     note: "Test case 1".to_string(),
        // },
        BenchConfig {
            tcfg: TransportConfig {
                use_mux: true,
                batch: Some(BatchConfig {
                    size_byte: 4096,
                    delay: Duration::from_millis(100),
                }),
            },
            client_cnt: 5,
            repeat_cnt: 3,
            server_thread_cnt: 1,
            client_thread_cnt: 5,
            last_time: Duration::from_secs(5),
            addl_payload_bytes: 0,
            note: "Test case 1".to_string(),
        },
    ];

    // let mut server = BenchServerManager::new(cases, "130.245.173.102:12345", "130.245.173.102:12346");
    let mut server = BenchServerManager::new(cases, "localhost:12345", "localhost:12346");

    let results = server.run().unwrap();
    println!("Benchmark results: {:?}", results);
}
