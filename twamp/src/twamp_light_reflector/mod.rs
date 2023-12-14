use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod reflector;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Configuration {
    #[validate(contains = "LIGHT")]
    pub mode: String,
    pub source_ip_address: SocketAddr,
    pub ref_wait: u64,
}

impl Configuration {
    pub fn new(source_ip_address: &SocketAddr, ref_wait: u64) -> Self {
        Self {
            mode: "LIGHT".to_string(),
            source_ip_address: *source_ip_address,
            ref_wait,
        }
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            mode: Default::default(),
            source_ip_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
            ref_wait: Default::default(),
        }
    }
}
