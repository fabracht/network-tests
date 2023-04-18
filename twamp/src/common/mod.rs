use common::{
    message::{Message, PacketResults},
    time::{DateTime, NtpTimestamp},
};
use message_macro::BeBytes;

pub const MIN_UNAUTH_PADDING: usize = 27;

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
    pub modes: u32,          // Supported modes bitmask
    pub challenge: [u8; 16], // Server's challenge
    pub salt: [u8; 16],      // Server's salt
    pub count: u32,          // Server's iteration count
    pub mbz: [u8; 12],       // Must be zero (MBZ) octets
}

// Define the TWAMP Control Message Types
#[derive(BeBytes, Debug, PartialEq, Clone)]
pub enum ControlMessageType {
    SetupResponse = 1,
    ServerStart = 2,
    SetupRequest = 3,
    ClientStart = 4,
    StopRequest = 5,
    StopAccept = 6,
    // ... add other control message types as needed
}

// Define the TWAMP Setup Request message struct
#[derive(BeBytes, Debug, PartialEq)]
pub struct SetupRequest {
    msg_type: ControlMessageType,
    mode: u32,
    key: [u8; 80],
    // ... add other fields as needed
}

// Define the TWAMP Client Start message struct
#[derive(BeBytes, Debug, PartialEq)]
pub struct ClientStart {
    msg_type: ControlMessageType,
    time: NtpTimestamp,
    timeout: u32,
    type_p: u8,
    // ... add other fields as needed
}

// Define the TWAMP Server Start message struct
#[derive(BeBytes, Debug, PartialEq)]
pub struct ServerStart {
    msg_type: ControlMessageType,
    time: NtpTimestamp,
    // ... add other fields as needed
}

// Define the TWAMP Stop Request message struct
#[derive(BeBytes, Debug, PartialEq)]
pub struct StopRequest {
    msg_type: ControlMessageType,
    accept: u8,
}

// Define the TWAMP Stop Accept message struct
#[derive(BeBytes, Debug, PartialEq)]
pub struct StopAccept {
    msg_type: ControlMessageType,
    accept: u8,
}
