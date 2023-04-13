use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod reflector;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Configuration {
    #[validate(contains = "LIGHT")]
    pub mode: String,
    pub source_ip_address: String,
}
