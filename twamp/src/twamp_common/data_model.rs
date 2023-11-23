#![allow(dead_code)]

use std::net::IpAddr;

// Define the enums

/// The state of the server control connection.
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

/// The modes of the TWAMP.
#[derive(Debug)]
pub enum TwampModes {
    // Define the modes
}

/// The fill mode of the padding.
#[derive(Debug)]
pub enum PaddingFillMode {
    // Define the fill modes
}

/// The state of the sender session.
#[derive(Debug)]
pub enum SenderSessionState {
    // Define the session states
}

/// The distribution of the packets.
#[derive(Debug)]
pub enum PacketDistribution {
    /// Periodic distribution with a fixed interval.
    Periodic { periodic_interval: f64 },
    /// Poisson distribution with an average interval and a maximum interval.
    Poisson {
        lambda: f64,
        max_interval: Option<f64>,
    },
}

// Define the structs

/// The control client.
#[derive(Debug)]
pub struct ControlClient {
    /// The administrative state of the control client.
    admin_state: bool,
    /// The control connections of the control client.
    ctrl_connection: Vec<CtrlConnection>,
}

/// The control connection.
#[derive(Debug)]
pub struct CtrlConnection {
    /// The name of the control connection.
    name: String,
    /// The IP address of the server.
    server_ip: IpAddr,
    /// The TCP port of the server.
    server_tcp_port: u16,
    /// The state of the server control connection.
    state: Option<ServerCtrlConnectionState>,
    /// The DSCP of the control packet.
    control_packet_dscp: Option<u8>,
    /// The selected mode of the TWAMP.
    selected_mode: Option<TwampModes>,
    /// The ID of the key.
    key_id: Option<String>,
    /// The count.
    count: Option<u8>,
    /// The maximum count exponent.
    max_count_exponent: Option<u8>,
    /// The salt.
    salt: Option<Vec<u8>>,
    /// The server IV.
    server_iv: Option<Vec<u8>>,
    /// The challenge.
    challenge: Option<Vec<u8>>,
}

/// The session sender.
#[derive(Debug)]
pub struct SessionSender {
    /// The administrative state of the session sender.
    admin_state: Option<bool>,
    /// The test sessions of the session sender.
    test_session: Vec<TestSession>,
}

/// The test session.
#[derive(Debug)]
pub struct TestSession {
    /// The name of the test session.
    name: String,
    /// The name of the control connection.
    ctrl_connection_name: Option<String>,
    /// The fill mode of the padding.
    fill_mode: Option<PaddingFillMode>,
    /// The number of packets.
    number_of_packets: u32,
    /// The distribution of the packets.
    packet_distribution: Option<PacketDistribution>,
    /// The state of the sender session.
    state: Option<SenderSessionState>,
    /// The number of sent packets.
    sent_packets: Option<u32>,
    /// The number of received packets.
    rcv_packets: Option<u32>,
    /// The sequence number of the last sent packet.
    last_sent_seq: Option<u32>,
    /// The sequence number of the last received packet.
    last_rcv_seq: Option<u32>,
}

/// The session reflector.
#[derive(Debug)]
pub struct SessionReflector {
    /// The administrative state of the session reflector.
    admin_state: Option<bool>,
    /// The refwait of the session reflector.
    refwait: Option<u32>,
    /// The test sessions of the session reflector.
    test_session: Vec<TestSessionReflector>,
}

/// The test session reflector.
#[derive(Debug, PartialEq, Clone)]
pub struct TestSessionReflector {
    /// The SID of the test session reflector.
    pub sid: Option<String>,
    /// The IP address of the sender.
    pub sender_ip: IpAddr,
    /// The UDP port of the sender.
    pub sender_udp_port: u16,
    /// The IP address of the reflector.
    pub reflector_ip: IpAddr,
    /// The UDP port of the reflector.
    pub reflector_udp_port: u16,
    /// The IP address of the parent connection client.
    pub parent_connection_client_ip: Option<IpAddr>,
    /// The TCP port of the parent connection client.
    pub parent_connection_client_tcp_port: Option<u16>,
    /// The IP address of the parent connection server.
    pub parent_connection_server_ip: Option<IpAddr>,
    /// The TCP port of the parent connection server.
    pub parent_connection_server_tcp_port: Option<u16>,
    /// The DSCP of the test packet.
    pub test_packet_dscp: Option<u8>,
    /// The number of sent packets.
    pub sent_packets: Option<u32>,
    /// The number of received packets.
    pub rcv_packets: Option<u32>,
    /// The sequence number of the last sent packet.
    pub last_sent_seq: Option<u32>,
    /// The sequence number of the last received packet.
    pub last_rcv_seq: Option<u32>,
}

impl TestSessionReflector {
    pub fn new() -> Self {
        Self {
            sid: todo!(),
            sender_ip: todo!(),
            sender_udp_port: todo!(),
            reflector_ip: todo!(),
            reflector_udp_port: todo!(),
            parent_connection_client_ip: todo!(),
            parent_connection_client_tcp_port: todo!(),
            parent_connection_server_ip: todo!(),
            parent_connection_server_tcp_port: todo!(),
            test_packet_dscp: todo!(),
            sent_packets: todo!(),
            rcv_packets: todo!(),
            last_sent_seq: todo!(),
            last_rcv_seq: todo!(),
        }
    }
}
