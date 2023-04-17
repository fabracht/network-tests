use common::host::Host;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod reflector;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Configuration {
    pub hosts: Vec<Host>,
    pub mode: String,
    pub source_ip_address: String,
    #[validate(range(min = 1, max = 3600))]
    pub duration: u64,
    #[validate(range(min = 1, max = 1000))]
    pub packet_interval: u64,
    #[validate(range(min = 0, max = 1024))]
    pub padding: usize,
}
