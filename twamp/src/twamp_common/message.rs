#![allow(dead_code)]

use bebytes::BeBytes;
use core::time::Duration;
use network_commons::{
    error::CommonError,
    time::{DateTime, NtpTimestamp},
};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::{
    net::{IpAddr, SocketAddr},
    ops::BitAnd,
};

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

impl Serialize for PacketResults {
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

pub trait Message {
    fn packet_results(&self) -> PacketResults;
}

/// Unauthenticated TWAMP message as defined
/// in [RFC4656 Section 4.1.2](https://www.rfc-editor.org/rfc/rfc4656#section-4.1.2)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone)]
pub struct SenderMessage {
    /// Sender sequence number
    pub sequence_number: u32,
    /// Timestamp
    pub timestamp: NtpTimestamp,
    /// Error estimate on timestamp
    pub error_estimate: ErrorEstimate,
    /// Payload of the packet to send
    pub padding: Vec<u8>,
}

impl Message for SenderMessage {
    fn packet_results(&self) -> PacketResults {
        PacketResults {
            sender_seq: self.sequence_number,
            reflector_seq: None,
            t1: DateTime::try_from(self.timestamp).unwrap(),
            t2: None,
            t3: None,
            t4: None,
        }
    }
}

/// Unauthenticated TWAMP message as defined
/// in [RFC5357 Section 4.2.1](https://www.rfc-editor.org/rfc/rfc5357.html#section-4.2.1)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone)]
#[repr(C)]
pub struct ReflectedMessage {
    /// Reflector sequence number
    pub reflector_sequence_number: u32,
    /// Timestamp
    pub timestamp: NtpTimestamp,
    /// Error estimate on the timestamp
    pub error_estimate: ErrorEstimate,
    /// Must be zero
    pub mbz1: u16,
    /// Receive timestamp
    pub receive_timestamp: NtpTimestamp,
    /// Sender sequence number
    pub sender_sequence_number: u32,
    /// Timestamp
    pub sender_timestamp: NtpTimestamp,
    /// Error estimate on timestamp
    pub sender_error_estimate: ErrorEstimate,
    /// Must be zero
    pub mbz2: u16,
    /// Time to live (TTL) field of the sender's IP header
    pub sender_ttl: u8,
    /// Payload of the packet to send
    pub padding: Vec<u8>,
}

impl Message for ReflectedMessage {
    fn packet_results(&self) -> PacketResults {
        PacketResults {
            sender_seq: self.sender_sequence_number,
            reflector_seq: Some(self.reflector_sequence_number),
            t1: DateTime::try_from(self.sender_timestamp).unwrap(),
            t2: DateTime::try_from(self.receive_timestamp).ok(),
            t3: DateTime::try_from(self.timestamp).ok(),
            t4: None,
        }
    }
}

/// Estimation on the error on a timestamp based
/// on synchronization method used [RFC4656 Section 4.1.2](https://www.rfc-editor.org/rfc/rfc4656#section-4.1.2)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone, Copy)]
pub struct ErrorEstimate {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    pub multiplier: u8,
}

// Define the TWAMP Server Greeting message struct
#[derive(BeBytes, Debug, Default)]
pub struct ServerGreeting {
    pub unused: [u8; 12],    // 12 unused octets (zeroes)
    pub modes: Modes,        // Supported modes bitmask
    pub challenge: [u8; 16], // Server's challenge
    pub salt: [u8; 16],      // Server's salt
    pub count: u32,          // Server's iteration count
    pub mbz: [u8; 12],       // Must be zero (MBZ) octets
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Mode {
    Closed = 0b0000,
    Unauthenticated = 0b0001,
    Authenticated = 0b0010,
    Encrypted = 0b0100,
}

#[derive(BeBytes, Debug, PartialEq, Clone, Copy, Default)]
pub struct Modes {
    pub bits: u8,
}

impl Modes {
    pub fn set(&mut self, mode: Mode) {
        self.bits |= mode as u8;
    }

    pub fn unset(&mut self, mode: Mode) {
        self.bits &= !(mode as u8);
    }

    pub fn is_set(&self, mode: Mode) -> bool {
        self.bits & (mode as u8) == mode as u8
    }
}

impl BitAnd for Modes {
    type Output = Modes;

    fn bitand(self, rhs: Self) -> Self::Output {
        Modes {
            bits: self.bits & rhs.bits,
        }
    }
}

// Define the TWAMP Client Setup Response message struct
#[derive(BeBytes, Debug, PartialEq, Clone)]
pub struct ClientSetupResponse {
    pub mode: Modes,
    pub key_id: [u8; 80],
    pub token: [u8; 64],
    pub client_iv: [u8; 16],
}

#[derive(BeBytes, Debug, PartialEq, Clone)]
pub enum AcceptFields {
    Ok = 0,
    Failure = 1,
    InternalError = 2,
    NotSupported = 3,
    PermanentResourceLimitation = 4,
    TemporaryResourceLimitation = 5,
}

// Define the TWAMP Control message struct used to negotiate sessions
#[derive(BeBytes, Debug)]
pub struct ControlMessage {
    pub control_command: TwampControlCommandNumber,
    pub mbz: [u8; 15],
    pub hmac: [u8; 16],
}

#[derive(BeBytes, Debug)]
pub struct StopNSessions {
    pub control_command: TwampControlCommandNumber,
    pub accept_field: AcceptFields,
    pub mbz1: [u8; 2],
    pub number_of_sessions: u32,
    pub mbz2: [u8; 8],
    pub hmac: [u8; 4],
}

#[derive(BeBytes, Debug)]
pub struct AcceptSessionMessage {
    pub accept: AcceptFields,
    pub mbz1: u8,
    pub port: u16,
    pub sid: [u8; 16],
    pub mvb2: [u8; 12],
    pub hmac: [u8; 16],
}

#[derive(BeBytes, Debug, PartialEq, Clone, Default)]
pub enum TwampControlCommandNumber {
    #[default]
    Forbidden = 1,
    StartSessions = 2,
    StopSessions = 3,
    RequestTwSession = 5,
    StartNSessions = 7,
    StartNAck = 8,
    StopNSessions = 9,
    StopNAck = 10,
}

// Define the Request-Tw-Session message struct
#[derive(BeBytes, Debug)]
pub struct RequestTwSession {
    pub request_type: TwampControlCommandNumber, // Request-Type
    #[U8(size(4), pos(0))]
    pub mbz1: u8,      // Must be zero (MBZ) quartet
    #[U8(size(4), pos(4))]
    pub ipvn: u8,      // IP version number (4 or 6)
    pub conf_sender: u8,                         // Conf-Sender
    pub conf_receiver: u8,                       // Conf-Receiver
    pub num_schedule_slots: u32,                 // Schedule-Slots
    pub num_packets: u32,                        // Packets
    pub sender_port: u16,                        // Sender-Port
    pub receiver_port: u16,                      // Receiver-Port
    pub sender_address: [u8; 16],                // Sender-Address
    pub receiver_address: [u8; 16],              // Receiver-Address
    pub sid: [u8; 16],                           // SID
    pub padding_length: [u8; 4],                 // Padding
    pub start_time: NtpTimestamp,                // NtpTimestamp
    pub timeout: u32,                            // Timeout
    pub type_p: u8,                              // Type-P
    pub mbz2: [u8; 8],                           // Must be zero (MBZ) octets
    pub hmac: [u8; 16],                          // HMAC
}

pub struct RequestTwSessionBuilder {
    request_type: Option<TwampControlCommandNumber>,
    ipvn: Option<u8>,
    conf_sender: Option<u8>,
    conf_receiver: Option<u8>,
    num_schedule_slots: Option<u32>,
    num_packets: Option<u32>,
    sender_port: Option<u16>,
    receiver_port: Option<u16>,
    sender_address: Option<IpAddr>,
    receiver_address: Option<IpAddr>,
    sid: Option<[u8; 16]>,
    padding_length: Option<[u8; 4]>,
    start_time: Option<NtpTimestamp>,
    timeout: Option<u32>,
    type_p: Option<u8>,
    hmac: Option<[u8; 16]>,
}

impl RequestTwSessionBuilder {
    pub fn new() -> RequestTwSessionBuilder {
        RequestTwSessionBuilder {
            request_type: None,
            ipvn: None,
            conf_sender: None,
            conf_receiver: None,
            num_schedule_slots: None,
            num_packets: None,
            sender_port: None,
            receiver_port: None,
            sender_address: None,
            receiver_address: None,
            sid: None,
            padding_length: None,
            start_time: None,
            timeout: None,
            type_p: None,
            hmac: None,
        }
    }

    pub fn request_type(mut self, request_type: TwampControlCommandNumber) -> Self {
        self.request_type = Some(request_type);
        self
    }

    pub fn ipvn(mut self, ipvn: u8) -> Self {
        self.ipvn = Some(ipvn);
        self
    }

    pub fn num_schedule_slots(mut self, num_schedule_slots: u32) -> Self {
        self.num_schedule_slots = Some(num_schedule_slots);
        self
    }

    pub fn num_packets(mut self, num_packets: u32) -> Self {
        self.num_packets = Some(num_packets);
        self
    }

    pub fn sender_port(mut self, sender_port: u16) -> Self {
        self.sender_port = Some(sender_port);
        self
    }

    pub fn receiver_port(mut self, receiver_port: u16) -> Self {
        self.receiver_port = Some(receiver_port);
        self
    }

    pub fn sender_address(mut self, sender_address: Option<IpAddr>) -> Self {
        self.sender_address = sender_address;
        self
    }

    pub fn receiver_address(mut self, receiver_address: Option<IpAddr>) -> Self {
        self.receiver_address = receiver_address;
        self
    }

    pub fn sid(mut self, sid: [u8; 16]) -> Self {
        self.sid = Some(sid);
        self
    }

    pub fn padding_length(mut self, padding_length: [u8; 4]) -> Self {
        self.padding_length = Some(padding_length);
        self
    }

    pub fn start_time(mut self, start_time: NtpTimestamp) -> Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn timeout(mut self, timeout: u32) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn type_p(mut self, type_p: u8) -> Self {
        self.type_p = Some(type_p);
        self
    }

    pub fn hmac(mut self, hmac: [u8; 16]) -> Self {
        self.hmac = Some(hmac);
        self
    }

    pub fn build(self) -> Result<RequestTwSession, CommonError> {
        if self.request_type.is_none() {
            return Err(CommonError::from("request_type is not set"));
        }
        if let Some(ipvn) = self.ipvn {
            match ipvn {
                4 => {
                    if let Some(IpAddr::V6(_)) = self.sender_address {
                        return Err(CommonError::from(
                            "Mismatch between ipvn and sender_address type",
                        ));
                    }
                    if let Some(IpAddr::V6(_)) = self.receiver_address {
                        return Err(CommonError::from(
                            "Mismatch between ipvn and receiver_address type",
                        ));
                    }
                }
                6 => {
                    if let Some(IpAddr::V4(_)) = self.sender_address {
                        return Err(CommonError::from(
                            "Mismatch between ipvn and sender_address type",
                        ));
                    }
                    if let Some(IpAddr::V4(_)) = self.receiver_address {
                        return Err(CommonError::from(
                            "Mismatch between ipvn and receiver_address type",
                        ));
                    }
                }
                _ => return Err(CommonError::from("Invalid ipvn, must be 4 or 6")),
            }
        }

        if self.start_time.is_none() {
            return Err(CommonError::from("start_time is not set"));
        }

        if self.type_p.is_none() {
            return Err(CommonError::from("type_p is not set"));
        }
        let sender_address = match self.sender_address {
            Some(IpAddr::V4(addr)) => {
                let mut bytes = [0u8; 16];
                bytes[12..16].copy_from_slice(&addr.octets());
                bytes
            }
            Some(IpAddr::V6(addr)) => addr.octets(),
            None => [0u8; 16],
        };

        let receiver_address = match self.receiver_address {
            Some(IpAddr::V4(addr)) => {
                let mut bytes = [0u8; 16];
                bytes[12..16].copy_from_slice(&addr.octets());
                bytes
            }
            Some(IpAddr::V6(addr)) => addr.octets(),
            None => [0u8; 16],
        };

        Ok(RequestTwSession {
            request_type: self.request_type.unwrap_or_default(),
            mbz1: 0,
            ipvn: self.ipvn.unwrap_or(0),
            // Both the Conf-Sender field and Conf-Receiver field MUST be set to 0 since the Session-Reflector will both receive and send packets
            conf_sender: 0,
            conf_receiver: 0,
            num_schedule_slots: self.num_schedule_slots.unwrap_or(0),
            num_packets: self.num_packets.unwrap_or(0),
            sender_port: self.sender_port.unwrap_or(0),
            receiver_port: self.receiver_port.unwrap_or(0),
            sender_address,
            receiver_address,
            sid: self.sid.unwrap_or([0; 16]),
            padding_length: self.padding_length.unwrap_or([0; 4]),
            start_time: self.start_time.unwrap(),
            timeout: self.timeout.unwrap_or(0),
            type_p: self.type_p.unwrap_or(0),
            mbz2: [0; 8],
            hmac: self.hmac.unwrap_or([0; 16]),
        })
    }
}
