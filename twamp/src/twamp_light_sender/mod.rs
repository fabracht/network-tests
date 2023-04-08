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
    pub collection_period: i64,
    #[validate(range(min = 1, max = 1000))]
    pub packet_interval: i64,
}

const NETWORK_PRECISION: i32 = 3;
