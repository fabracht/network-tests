use common::host::Host;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod control;
pub mod control_session;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Configuration {
    #[validate(contains = "FULL")]
    pub mode: String,
    pub source_ip_address: String,
    pub ref_wait: u64,
}
