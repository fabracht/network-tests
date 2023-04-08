use crate::{
    twamp_light_reflector::reflector::Reflector,
    twamp_light_reflector::Configuration as ReflectorConfiguration,
    twamp_light_sender::{result::TwampResult, twamp_light::TwampLight},
};
use common::{host::Host, Strategy};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]

pub struct TwampConfiguration {
    pub hosts: Option<Vec<Host>>,
    pub mode: String,
    pub source_ip_address: Option<String>,
    pub collection_period: Option<u64>,
    pub packet_interval: Option<u64>,
}

pub struct Twamp {
    configuration: TwampConfiguration,
}

impl Twamp {
    pub fn new(configuration: TwampConfiguration) -> Self {
        Self { configuration }
    }

    pub fn generate(
        &self,
    ) -> Result<Box<dyn Strategy<TwampResult, crate::CommonError>>, crate::CommonError> {
        let hosts = self
            .configuration
            .hosts
            .iter()
            .flat_map(|host| host.clone())
            .collect::<Vec<Host>>();
        match self.configuration.mode.as_str() {
            "LIGHT_SENDER" => Ok(Box::new(TwampLight::new(
                &self
                    .configuration
                    .source_ip_address
                    .clone()
                    .unwrap_or_default(),
                Duration::from_secs(self.configuration.collection_period.unwrap_or_default()),
                &hosts,
                Duration::from_millis(
                    self.configuration
                        .packet_interval
                        .unwrap_or_default()
                        .into(),
                ),
            ))),
            "LIGHT_REFLECTOR" => Ok(Box::new(Reflector::new(ReflectorConfiguration {
                mode: self.configuration.mode.clone(),
                source_ip_address: self
                    .configuration
                    .clone()
                    .source_ip_address
                    .unwrap_or_default()
                    .clone(),
            }))),
            _ => panic!("No such mode"),
        }
    }
}
