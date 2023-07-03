use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

use crate::error::CommonError;

/// `Host` defines a network host with an IP and port number.
/// It supports serialization/deserialization, comparison, cloning, and validation.
#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct Host {
    pub ip: String,
    pub port: u16,
}

/// `TryFrom<&Host>` allows conversion from a `Host` to a `SocketAddr`.
/// It returns a `CommonError` if the IP string is not valid.
impl TryFrom<&Host> for SocketAddr {
    type Error = CommonError;

    fn try_from(value: &Host) -> Result<Self, Self::Error> {
        let ip = value.ip.parse()?;
        Ok(Self::new(ip, value.port))
    }
}
