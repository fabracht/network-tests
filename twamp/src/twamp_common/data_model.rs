use std::{net::SocketAddr, ops::BitAnd, time::Duration};

use bebytes::BeBytes;
use network_commons::time::DateTime;
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};

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

#[derive(BeBytes, Debug, PartialEq, Clone, Copy)]
pub enum Mode {
    Closed = 0b0000,
    Unauthenticated = 0b0001,
    Authenticated = 0b0010,
    Encrypted = 0b0100,
}

impl From<u8> for Mode {
    fn from(value: u8) -> Self {
        match value {
            0b0000 => Mode::Closed,
            0b0001 => Mode::Unauthenticated,
            0b0010 => Mode::Authenticated,
            0b0100 => Mode::Encrypted,
            _ => Mode::Closed,
        }
    }
}

impl From<Mode> for u8 {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Closed => 0b0000,
            Mode::Unauthenticated => 0b0001,
            Mode::Authenticated => 0b0010,
            Mode::Encrypted => 0b0100,
        }
    }
}

#[derive(BeBytes, Debug, PartialEq, Clone, Copy, Default)]
pub struct Modes {
    pub bits: u8,
}

impl Modes {
    pub fn set(&mut self, mode: Mode) {
        self.bits |= mode as u8;
    }

    pub fn _unset(&mut self, mode: Mode) {
        self.bits &= !(mode as u8);
    }

    pub fn _is_set(&self, mode: Mode) -> bool {
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

#[derive(BeBytes, Debug, PartialEq, Clone)]
pub enum AcceptFields {
    Ok = 0,
    Failure = 1,
    InternalError = 2,
    NotSupported = 3,
    PermanentResourceLimitation = 4,
    TemporaryResourceLimitation = 5,
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////
#[non_exhaustive]
#[derive(BeBytes, Debug, PartialEq, Clone, Default)]
pub enum TwampControlCommand {
    #[default]
    Forbidden = 1,
    StartSessions = 2,
    StopSessions = 3,
    RequestTwSession = 5,
    StartNSessions = 7,
    StartNAck = 8,
    StopNSessions = 9,
    StopNAck = 10,
    Other,
}

/// The state of the server control connection.
#[allow(dead_code)]
#[derive(Debug)]
pub enum ServerCtrlConnectionState {
    Greeting,
    Authentication,
    Negotiation,
    Start,
    Monitor,
    End,
    Retry,
    Error,
}

/// The state of the sender session.
#[allow(dead_code)]
#[derive(Debug)]
pub enum SenderSessionState {
    AwaitingServerGreeting,
    SendingClientSetup,
    AwaitingServerStart,
    SendingRequestSession,
    AwaitingSessionAcceptance,
    SessionEstablished,
    AwaitingStartAck,
    TestInProgress,
    SessionRefused,
    ClosingConnection,
    FinalState,
}

/// The control connection.
#[derive(Debug)]
pub struct CtrlConnection {
    /// The name of the control connection.
    pub name: String,
    /// The socket address of the client.
    pub client_socket_addr: SocketAddr,
    /// The IP address of the server.
    pub server_socket_address: SocketAddr,
    /// The state of the server control connection.
    pub state: Option<ServerCtrlConnectionState>,
    /// The DSCP of the control packet.
    pub control_packet_dscp: Option<u8>,
    /// The selected mode of the TWAMP.
    pub selected_mode: Option<Mode>,
    /// The ID of the key.
    pub key_id: Option<String>,
    /// The count.
    pub count: Option<u8>,
    /// The maximum count exponent.
    pub max_count_exponent: Option<u8>,
    /// The salt.
    pub salt: Option<Vec<u8>>,
    /// The server IV.
    pub server_iv: Option<Vec<u8>>,
    /// The challenge.
    pub challenge: Option<Vec<u8>>,
}

// /// The test session reflector.
// #[derive(Debug, PartialEq, Clone)]
// pub struct TestSessionReflector {
//     /// The SID of the test session reflector.
//     pub sid: String,
//     /// The socket address of the sender.
//     pub sender_address: Option<SocketAddr>,
//     /// The socket address of the reflector.
//     pub reflector_address: SocketAddr,
//     /// The socket address of the parent connection client.
//     pub parent_connection_client_address: Option<SocketAddr>,
//     /// The IP address of the parent connection server.
//     pub parent_connection_server_address: Option<SocketAddr>,
//     /// The DSCP of the test packet.
//     pub test_packet_dscp: Option<u8>,
//     /// The number of sent packets.
//     pub sent_packets: u32,
//     /// The number of received packets.
//     pub rcv_packets: u32,
//     /// The sequence number of the last sent packet.
//     pub last_sent_seq: u32,
//     /// The sequence number of the last received packet.
//     pub last_rcv_seq: u32,
// }

// impl TestSessionReflector {
//     pub fn new(sid: &str, configuration: Configuration) -> Self {
//         Self {
//             sid: sid.to_string(),
//             sender_address: None,
//             reflector_address: configuration.source_ip_address,
//             parent_connection_client_address: None,
//             parent_connection_server_address: None,
//             test_packet_dscp: None,
//             sent_packets: 0,
//             rcv_packets: 0,
//             last_sent_seq: 0,
//             last_rcv_seq: 0,
//         }
//     }

//     pub fn create_udp_socket(&mut self) -> Result<TimestampedUdpSocket, CommonError> {
//         let socket = mio::net::UdpSocket::bind(self.reflector_address)?;
//         let mut my_socket = TimestampedUdpSocket::new(socket.into_raw_fd());
//         my_socket.set_fcntl_options()?;
//         my_socket.set_socket_options(libc::SOL_IP, libc::IP_RECVERR, Some(1))?;
//         my_socket.set_timestamping_options()?;

//         Ok(my_socket)
//     }
// }
