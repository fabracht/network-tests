use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod reflector;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Configuration {
    #[validate(contains = "LIGHT")]
    pub mode: String,
    pub source_ip_address: String,
    pub ref_wait: u64,
}

impl Configuration {
    pub fn new(source_ip_address: &str, ref_wait: u64) -> Self {
        Self {
            mode: "LIGHT".to_string(),
            source_ip_address: source_ip_address.to_owned(),
            ref_wait,
        }
    }
}
