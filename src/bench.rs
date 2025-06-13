use crate::{
    common::{Config, BatchConfig}
};

struct BenchConfig {
    pub cfg : Config, 
    pub ip : String, 
    pub port : u16, 
    pub client_cnt : usize, 
    
}