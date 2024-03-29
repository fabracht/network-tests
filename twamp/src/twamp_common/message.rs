#![allow(dead_code)]

use std::net::IpAddr;

use bebytes::BeBytes;
use network_commons::{
    error::CommonError,
    time::{DateTime, NtpTimestamp},
};

use super::{
    data_model::{AcceptFields, ErrorEstimate, Message, Modes, PacketResults, TwampControlCommand},
    MIN_UNAUTH_PADDING,
};

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

// Define the TWAMP Client Setup Response message struct
#[derive(BeBytes, Debug, PartialEq, Clone)]
pub struct ClientSetupResponse {
    pub mode: Modes,
    pub key_id: [u8; 80],
    pub token: [u8; 64],
    pub client_iv: [u8; 16],
}

// Define the TWAMP Server Start message struct
#[derive(BeBytes, Debug, PartialEq, Clone)]
pub struct ServerStart {
    pub mbz1: [u8; 15],           // Server's nonce
    pub accept: AcceptFields,     // Acceptance indicator (true if the server accepts the session)
    pub server_iv: [u8; 16],      // Server's nonce
    pub start_time: NtpTimestamp, // Server's identity, encrypted with the client's public key (optional)
    pub mbz2: [u8; 8],            // Server's nonce
}

// Define the TWAMP Control message struct used to negotiate sessions
#[derive(BeBytes, Debug)]
pub struct ControlMessage {
    pub control_command: u8,
    pub mbz: [u8; 15],
    pub hmac: [u8; 16],
}

#[derive(BeBytes, Debug)]
pub struct StopNSessions {
    pub control_command: TwampControlCommand,
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

// Define the Request-Tw-Session message struct
#[derive(BeBytes, Debug)]
pub struct RequestTwSession {
    pub request_type: TwampControlCommand, // Request-Type
    #[U8(size(4), pos(0))]
    pub mbz1: u8, // Must be zero (MBZ) quartet
    #[U8(size(4), pos(4))]
    pub ipvn: u8, // IP version number (4 or 6)
    pub conf_sender: u8,                   // Conf-Sender
    pub conf_receiver: u8,                 // Conf-Receiver
    pub num_schedule_slots: u32,           // Schedule-Slots
    pub num_packets: u32,                  // Packets
    pub sender_port: u16,                  // Sender-Port
    pub reflector_port: u16,               // Receiver-Port as per RFC5357
    pub sender_address: [u8; 16],          // Sender-Address
    pub reflector_address: [u8; 16],       // Receiver-Address as per RFC5357
    pub sid: [u8; 16],                     // SID
    pub padding_length: u32,               // Padding
    pub start_time: NtpTimestamp,          // NtpTimestamp
    pub timeout: u32,                      // Timeout
    pub type_p: u8,                        // Type-P
    pub mbz2: [u8; 8],                     // Must be zero (MBZ) octets
    pub hmac: [u8; 16],                    // HMAC
}

pub struct RequestTwSessionBuilder {
    request_type: Option<TwampControlCommand>,
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
    padding_length: Option<u32>,
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

    pub fn request_type(mut self, request_type: TwampControlCommand) -> Self {
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

    pub fn sender_address(mut self, sender_address: IpAddr) -> Self {
        self.sender_address = Some(sender_address);
        self
    }

    pub fn receiver_address(mut self, receiver_address: IpAddr) -> Self {
        self.receiver_address = Some(receiver_address);
        self
    }

    pub fn sid(mut self, sid: [u8; 16]) -> Self {
        self.sid = Some(sid);
        self
    }

    pub fn padding_length(mut self, padding_length: u32) -> Self {
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
            conf_sender: 0,   // set to 0 as per RFC5357
            conf_receiver: 0, // set to 0 as per RFC5357
            num_schedule_slots: self.num_schedule_slots.unwrap_or(0),
            num_packets: self.num_packets.unwrap_or(0),
            sender_port: self.sender_port.unwrap_or(0),
            reflector_port: self.receiver_port.unwrap_or(0),
            sender_address,
            reflector_address: receiver_address,
            sid: self.sid.unwrap_or([0; 16]),
            padding_length: self.padding_length.unwrap_or(MIN_UNAUTH_PADDING as u32),
            start_time: self.start_time.unwrap(),
            timeout: self.timeout.unwrap_or(0),
            type_p: self.type_p.unwrap_or(0),
            mbz2: [0; 8],
            hmac: self.hmac.unwrap_or([0; 16]),
        })
    }
}
