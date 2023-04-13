use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

use crate::error::CommonError;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct Host {
    pub ip: String,
    pub port: u16,
}

impl TryFrom<&Host> for SocketAddr {
    type Error = CommonError;

    fn try_from(value: &Host) -> Result<Self, Self::Error> {
        let ip = value.ip.parse()?;
        Ok(Self::new(ip, value.port))
    }
}
