use std::net::SocketAddr;

use network_commons::{
    epoll_loop::EventLoopMessages, error::CommonError, udp_socket::TimestampedUdpSocket,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod control;
pub mod control_client;
pub mod control_session;
#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ControlConfiguration {
    #[validate(contains = "FULL")]
    pub mode: String,
    pub source_ip_address: SocketAddr,
    pub ref_wait: u64,
}

impl ControlConfiguration {
    pub fn new(mode: &str, source_ip_address: &SocketAddr, ref_wait: u64) -> Self {
        Self {
            mode: mode.to_owned(),
            source_ip_address: source_ip_address.to_owned(),
            ref_wait,
        }
    }
}

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ClientConfiguration {
    #[validate(contains = "FULL")]
    pub mode: String,
    pub source_ip_address: SocketAddr,
    pub target_ip_address: SocketAddr,
    pub ref_wait: u64,
}

impl ClientConfiguration {
    pub fn new(
        mode: &str,
        source_ip_address: &SocketAddr,
        target_ip_address: &SocketAddr,
        ref_wait: u64,
    ) -> Self {
        Self {
            mode: mode.to_owned(),
            source_ip_address: source_ip_address.to_owned(),
            target_ip_address: target_ip_address.to_owned(),
            ref_wait,
        }
    }
}

pub type WorkerSender = std::sync::mpsc::Sender<
    EventLoopMessages<(
        TimestampedUdpSocket,
        Box<
            dyn FnMut(
                    &mut TimestampedUdpSocket,
                    network_commons::event_loop::Token,
                ) -> Result<i32, CommonError>
                + Send,
        >,
    )>,
>;
