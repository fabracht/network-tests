use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod reflector;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Configuration {
    #[validate(contains = "FULL")]
    pub mode: String,
    pub source_ip_address: String,
}
