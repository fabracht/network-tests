use common::host::Host;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod result;
pub mod twamp_light;

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

const NETWORK_PRECISION: i32 = 3;

impl Configuration {
    pub fn new(
        hosts: &[Host],
        mode: &str,
        source_ip_address: &str,
        duration: u64,
        packet_interval: u64,
        padding: usize,
    ) -> Self {
        Self {
            hosts: hosts.to_owned(),
            mode: mode.into(),
            source_ip_address: source_ip_address.to_owned(),
            duration,
            packet_interval,
            padding,
        }
    }
}
