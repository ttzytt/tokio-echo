use tokio_echo::{
    bench::client::BenchClientManager
};
use tokio;
use std::env;

fn main(){
    tracing_subscriber::fmt::init();
    unsafe { env::set_var("RUST_BACKTRACE", "full") };

    // let mut client = BenchClientManager::new(
    //     "localhost:12345",
    //     "localhost:12346"
    // );

    let mut client = BenchClientManager::new(
        "130.245.173.102:12345",
        "130.245.173.102:12346"
    );


    client.run().unwrap();
}