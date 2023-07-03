//! This module provides handling for generic messages that involve the exchange of
//! four timestamps (t1, t2, t3, t4), which are common in various network measurement protocols.
use crate::time::DateTime;
use core::time::Duration;
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::net::SocketAddr;

/// `Message` is a trait that requires implementation of `packet_results()` function.
pub trait Message {
    fn packet_results(&self) -> PacketResults;
}

/// `PacketResults` represents a generic message with four timestamps.
/// Fields that might not be available are optional.
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct PacketResults {
    pub sender_seq: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflector_seq: Option<u32>,
    pub t1: DateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t2: Option<DateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t3: Option<DateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t4: Option<DateTime>,
}

impl serde::Serialize for PacketResults {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("PacketResults", 6)?;
        s.serialize_field("sender_seq", &self.sender_seq)?;
        s.serialize_field("reflector_seq", &self.reflector_seq)?;
        s.serialize_field("t1", &self.t1)?;
        s.serialize_field("t2", &self.t2)?;
        s.serialize_field("t3", &self.t3)?;
        s.serialize_field("t4", &self.t4)?;
        s.end()
    }
}

impl PacketResults {
    pub fn calculate_rtt(&self) -> Option<Duration> {
        Some(self.t4? - self.t1)
    }
    pub fn calculate_owd_forward(&self) -> Option<Duration> {
        let duration = self.t2? - self.t1;
        log::debug!("OWD Forward Duration: {:?}", duration);

        Some(duration)
    }
    pub fn calculate_owd_backward(&self) -> Option<Duration> {
        let duration = self.t4? - self.t3?;
        log::debug!("OWD Backward Duration: {:?}", duration);
        Some(duration)
    }
    /// Calculates the Remote Processing Delay, which is the time the packet took to be processed on the server
    pub fn calculate_rpd(&self) -> Option<Duration> {
        Some(self.t3? - self.t2?)
    }
}

/// `SessionPackets` holds the address and optionally the packets of a test session.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionPackets {
    pub address: SocketAddr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets: Option<Vec<PacketResults>>,
}

/// `TimestampsResult` is the result of a test session, including an error string if there was an issue.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimestampsResult {
    pub session: SessionPackets,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
