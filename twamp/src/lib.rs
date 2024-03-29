use std::net::SocketAddr;

use crate::twamp_light_reflector::reflector::Reflector;
use crate::twamp_light_reflector::Configuration as ReflectorConfiguration;

use crate::twamp_light_sender::Configuration as LightConfiguration;
use network_commons::{error::CommonError, Strategy};
use serde::{Deserialize, Serialize};
use twamp_control::{
    control::Control, control_client::ControlClient, ClientConfiguration, ControlConfiguration,
};
pub use twamp_light_sender::result::TwampResult;
use twamp_light_sender::twamp_light::SessionSender;
use validator::Validate;

mod twamp_common;
mod twamp_control;
mod twamp_light_reflector;
mod twamp_light_sender;
#[derive(Validate, Serialize, Deserialize, Debug, PartialEq, Clone, Default)]

pub struct TwampConfiguration {
    pub test_session_hosts: Option<Vec<SocketAddr>>,
    pub control_host: Option<SocketAddr>,
    pub mode: String,
    pub source_ip_address: Option<String>,
    pub collection_period: Option<u64>,
    pub packet_interval: Option<u64>,
    pub padding: Option<usize>,
    pub last_message_timeout: Option<u64>,
    pub ref_wait: Option<u64>,
}

pub struct Twamp {
    configuration: TwampConfiguration,
}

impl Twamp {
    pub fn new(configuration: TwampConfiguration) -> Self {
        Self { configuration }
    }

    pub fn generate(&self) -> Result<Box<dyn Strategy<TwampResult, CommonError>>, CommonError> {
        let test_session_hosts = self
            .configuration
            .test_session_hosts
            .iter()
            .flat_map(|host| host.clone())
            .collect::<Vec<SocketAddr>>();
        let source_ip = self
            .configuration
            .clone()
            .source_ip_address
            .unwrap_or("0.0.0.0:0".to_string())
            .parse()?;
        match self.configuration.mode.as_str() {
            "LIGHT_SENDER" => {
                let configuration = LightConfiguration::new(
                    &test_session_hosts,
                    &source_ip,
                    self.configuration.collection_period.unwrap_or_default(),
                    self.configuration.packet_interval.unwrap_or_default(),
                    self.configuration.padding.unwrap_or_default(),
                    self.configuration.last_message_timeout.unwrap_or_default(),
                );
                configuration
                    .validate()
                    .map_err(CommonError::ValidationError)?;
                let twamp_light = SessionSender::new(&configuration);
                Ok(Box::new(twamp_light))
            }
            "LIGHT_REFLECTOR" => {
                let configuration = ReflectorConfiguration::new(
                    &source_ip,
                    self.configuration.ref_wait.unwrap_or(900),
                );
                configuration
                    .validate()
                    .map_err(CommonError::ValidationError)?;
                Ok(Box::new(Reflector::new(configuration)))
            }
            "FULL_SENDER" => {
                let control_configuration = ClientConfiguration::new(
                    &self.configuration.mode,
                    &source_ip,
                    self.configuration.control_host.as_ref().unwrap(),
                );
                let sesssion_configuration = LightConfiguration::new(
                    &test_session_hosts,
                    &source_ip,
                    self.configuration.collection_period.unwrap_or_default(),
                    self.configuration.packet_interval.unwrap_or_default(),
                    self.configuration.padding.unwrap_or_default(),
                    self.configuration.last_message_timeout.unwrap_or_default(),
                );
                Ok(Box::new(ControlClient::new(
                    &control_configuration,
                    &sesssion_configuration,
                )))
            }
            "FULL_REFLECTOR" => {
                let configuration = ControlConfiguration {
                    source_ip_address: self
                        .configuration
                        .clone()
                        .source_ip_address
                        .unwrap_or("0.0.0.0:0".to_string())
                        .parse()?,
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
