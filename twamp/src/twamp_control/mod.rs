use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod control;
pub mod control_client;
pub mod control_client_session;
pub mod control_session;
#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ControlConfiguration {
    pub source_ip_address: SocketAddr,
    pub ref_wait: u64,
}

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ClientConfiguration {
    #[validate(contains = "FULL")]
    pub mode: String,
    pub control_host: SocketAddr,
    pub source_address: SocketAddr,
}

impl ClientConfiguration {
    pub fn new(mode: &str, source_ip_address: &SocketAddr, control_host: &SocketAddr) -> Self {
        Self {
            mode: mode.to_owned(),
            source_address: source_ip_address.to_owned(),
            control_host: control_host.to_owned(),
        }
    }
}
