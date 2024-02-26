use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod result;
pub mod twamp_light;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Configuration {
    pub hosts: Vec<SocketAddr>,
    pub source_ip_address: SocketAddr,
    #[validate(range(min = 1, max = 3600))]
    pub duration: u64,
    #[validate(range(min = 1, max = 1000))]
    pub packet_interval: u64,
    #[validate(range(min = 0, max = 1024))]
    pub padding: usize,
    #[validate(range(min = 0, max = 1000))]
    pub last_message_timeout: u64,
}

const NETWORK_PRECISION: i32 = 0;

impl Configuration {
    pub fn new(
        hosts: &[SocketAddr],
        source_ip_address: &SocketAddr,
        duration: u64,
        packet_interval: u64,
        padding: usize,
        last_message_timeout: u64,
    ) -> Self {
        Self {
            hosts: hosts.to_owned(),
            source_ip_address: source_ip_address.to_owned(),
            duration,
            packet_interval,
            padding,
            last_message_timeout,
        }
    }
}
