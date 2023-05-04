use crate::{
    twamp_control::{control::Control, Configuration as FullReflectorConfiguration},
    twamp_light_reflector::reflector::Reflector,
    twamp_light_reflector::Configuration as ReflectorConfiguration,
    twamp_light_sender::{
        result::TwampResult, twamp_light::TwampLight, Configuration as LightConfiguration,
    },
};
use common::{error::CommonError, host::Host, Strategy};

use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]

pub struct TwampConfiguration {
    pub hosts: Option<Vec<Host>>,
    pub mode: String,
    pub source_ip_address: Option<String>,
    pub collection_period: Option<u64>,
    pub packet_interval: Option<u64>,
    pub padding: Option<usize>,
    pub last_message_timeout: Option<u64>,
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
            "LIGHT_SENDER" => {
                let configuration = LightConfiguration::new(
                    &hosts,
                    "LIGHT",
                    &self
                        .configuration
                        .source_ip_address
                        .clone()
                        .unwrap_or_default(),
                    self.configuration.collection_period.unwrap_or_default(),
                    self.configuration.packet_interval.unwrap_or_default(),
                    self.configuration.padding.unwrap_or_default(),
                    self.configuration.last_message_timeout.unwrap_or_default(),
                );
                configuration
                    .validate()
                    .map_err(CommonError::ValidationError)?;
                let twamp_light = TwampLight::new(&configuration);
                Ok(Box::new(twamp_light))
            }
            "LIGHT_REFLECTOR" => {
                let configuration = ReflectorConfiguration {
                    mode: self.configuration.mode.clone(),
                    source_ip_address: self
                        .configuration
                        .clone()
                        .source_ip_address
                        .unwrap_or_default(),
                    ref_wait: self.configuration.last_message_timeout.unwrap_or(900),
                };
                configuration
                    .validate()
                    .map_err(CommonError::ValidationError)?;
                Ok(Box::new(Reflector::new(configuration)))
            }
            "FULL_SENDER" => unimplemented!(),
            "FULL_REFLECTOR" => {
                let configuration = FullReflectorConfiguration {
                    mode: self.configuration.mode.clone(),
                    source_ip_address: self
                        .configuration
                        .clone()
                        .source_ip_address
                        .unwrap_or_default(),
                    ref_wait: self.configuration.last_message_timeout.unwrap_or(900),
                };
                configuration
                    .validate()
                    .map_err(CommonError::ValidationError)?;
                Ok(Box::new(Control::new(configuration)))
            }
            _ => panic!("No such mode"),
        }
    }
}
