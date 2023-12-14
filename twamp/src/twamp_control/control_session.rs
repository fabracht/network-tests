#![allow(dead_code)]
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use crate::twamp_common::data_model::AcceptFields;
use crate::twamp_common::data_model::ErrorEstimate;
use crate::twamp_common::data_model::Modes;
use crate::twamp_common::data_model::ServerCtrlConnectionState;
use crate::twamp_common::MIN_UNAUTH_PADDING;
// use crate::twamp_common::data_model::TestSessionReflector;

use crate::twamp_common::data_model::TwampControlCommand;
use crate::twamp_common::message::AcceptSessionMessage;
use crate::twamp_common::message::ClientSetupResponse;

use crate::twamp_common::message::ControlMessage;
use crate::twamp_common::message::ReflectedMessage;
use crate::twamp_common::message::RequestTwSession;
use crate::twamp_common::message::SenderMessage;
use crate::twamp_common::message::ServerGreeting;
use crate::twamp_common::message::ServerStart;
use crate::twamp_common::session::Session;

use bebytes::BeBytes;

use network_commons::epoll_loop::DuplexChannel;
use network_commons::epoll_loop::EventLoopMessages;
use network_commons::error::CommonError;
use network_commons::time::DateTime;
use network_commons::time::NtpTimestamp;
use network_commons::udp_socket::TimestampedUdpSocket;
use network_commons::{socket::Socket, tcp_socket::TimestampedTcpSocket};

// Define a struct to represent the TWAMP control session
pub struct ControlSession {
    pub id: i32,
    supported_modes: Modes,
    state: ServerCtrlConnectionState,
    twamp_sessions: Arc<RwLock<Vec<Session>>>,
    retry_count: u32, // Number of times to retry failed steps
    error_count: u32, // Number of times to tolerate errors before terminating the session
    auth_timeout: std::time::Duration,
    negotiation_timeout: std::time::Duration,
    start_timeout: std::time::Duration,
    monitor_timeout: std::time::Duration,
    rx_buffer: [u8; 1 << 16],
    worker_event_sender: Arc<Mutex<DuplexChannel<TimestampedUdpSocket>>>,
    start_time: DateTime,
}

impl ControlSession {
    // Method to create a new TWAMP control session with the initial state and TCP connection
    pub fn new(
        token: i32,
        mode: Modes,
        retry_count: u32,
        error_count: u32,
        worker_event_sender: Arc<Mutex<DuplexChannel<TimestampedUdpSocket>>>,
    ) -> ControlSession {
        let start_time = DateTime::utc_now();

        ControlSession {
            id: token,
            supported_modes: mode,
            state: ServerCtrlConnectionState::Greeting,
            twamp_sessions: Arc::new(RwLock::new(Vec::new())),
            retry_count,
            error_count,
            auth_timeout: std::time::Duration::from_secs(30),
            negotiation_timeout: std::time::Duration::from_secs(30),
            start_timeout: std::time::Duration::from_secs(10),
            monitor_timeout: std::time::Duration::from_secs(10),
            rx_buffer: [0; 1 << 16],
            worker_event_sender,
            start_time,
        }
    }

    // Method to transition to the next state of the state machine
    pub fn transition(&mut self, socket: &mut TimestampedTcpSocket) -> Result<(), CommonError> {
        match self.state {
            ServerCtrlConnectionState::Greeting => {
                let server_greeting = ServerGreeting::new(
                    [0; 12],
                    self.supported_modes,
                    [0; 16],
                    [0; 16],
                    1,
                    [0; 12],
                );

                log::info!("Sending Greeting message");
                let result = socket.send(server_greeting);
                match result {
                    // If successful, transition to the authentication state
                    Ok((_result, _)) => {
                        log::info!("Transition to Authentication");
                        self.state = ServerCtrlConnectionState::Negotiation
                    }
                    // If failed, transition to the error state or retry state
                    Err(_e) => {
                        return Err(CommonError::Generic(
                            "Error sending Greeting response".to_string(),
                        ));
                    }
                }
            }
            ServerCtrlConnectionState::Authentication => {
                log::info!("Authenticating");

                self.state = ServerCtrlConnectionState::Negotiation;
            }
            ServerCtrlConnectionState::Negotiation => {
                let result = socket.receive(&mut self.rx_buffer);
                if let Ok(result) = result {
                    if result.0 != 0 {
                        log::info!("Received ClientSetupResponse");
                        match ClientSetupResponse::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                // verify if the mode requested is supported
                                if response.mode & self.supported_modes == response.mode {
                                    let server_start = ServerStart {
                                        mbz1: [0u8; 15],                    // Server's nonce
                                        accept: AcceptFields::Ok, // Acceptance indicator (true if the server accepts the session)
                                        server_iv: [0u8; 16],     // Server's nonce
                                        start_time: self.start_time.into(), // Server's identity, encrypted with the client's lic ke0y (optional)
                                        mbz2: [0u8; 8],                     // Server's nonce
                                    };
                                    let result = socket.send(server_start);
                                    match result {
                                        // If successful, transition to the authentication state
                                        Ok((_result, _)) => {
                                            log::info!("Transition to Monitor");
                                            self.state = ServerCtrlConnectionState::Monitor;
                                        }
                                        // If failed, transition to the error state or retry state
                                        Err(_e) => {
                                            return Err(CommonError::Generic(
                                                "Error sending Greeting response".to_string(),
                                            ));
                                        }
                                    }
                                } else {
                                    return Err(CommonError::Generic(
                                        "Mode not supported".to_string(),
                                    ));
                                }
                            }
                            Err(_) => {
                                log::error!("Can't parse Greeting bytes");
                                return Err(CommonError::Generic(
                                    "Error parsing Greeting response".to_string(),
                                ));
                            }
                        };
                    } else {
                        log::error!("Can't receive 0 bytes");
                        return Err(CommonError::Generic("Close signal received".to_string()));
                    }
                }
            }
            ServerCtrlConnectionState::Monitor => {
                let result = socket.receive(&mut self.rx_buffer);
                log::info!("Received message in Monitor");
                if let Ok(result) = result {
                    if result.0 != 0 {
                        match RequestTwSession::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                match response.request_type {
                                    TwampControlCommand::Forbidden => {
                                        println!("Forbidden!");
                                    }
                                    TwampControlCommand::StartSessions => {
                                        // Start sessions
                                        self.state = ServerCtrlConnectionState::Start;
                                        self.transition(socket)?;
                                    }
                                    TwampControlCommand::StopSessions => {
                                        // We must unregister the sessions socket from the event loop and cleanup
                                        log::info!("Received StopSessions");
                                        let _ = self
                                            .worker_event_sender
                                            .try_lock()?
                                            .send(EventLoopMessages::Clean);
                                    }
                                    TwampControlCommand::RequestTwSession => {
                                        log::info!("Received RequestTwSession");
                                        // Check if port is already in use, if not, propose the next available
                                        let response_ip = response.reflector_address;
                                        let response_port = response.reflector_port;
                                        let response_sender_ip = response.sender_address;
                                        let response_sender_port = response.sender_port;
                                        let source_address = SocketAddr::V4(SocketAddrV4::new(
                                            Ipv4Addr::new(
                                                response_ip[0],
                                                response_ip[1],
                                                response_ip[2],
                                                response_ip[3],
                                            ),
                                            response_port,
                                        ));
                                        let sender_address = SocketAddr::V4(SocketAddrV4::new(
                                            Ipv4Addr::new(
                                                response_sender_ip[0],
                                                response_sender_ip[1],
                                                response_sender_ip[2],
                                                response_sender_ip[3],
                                            ),
                                            response_sender_port,
                                        ));

                                        let mut sessions_lock = self.twamp_sessions.write()?;
                                        let mut session_iter = sessions_lock.iter_mut();
                                        let mut session_option = session_iter.find(|session| {
                                            session.rx_socket_address.port()
                                                == response.reflector_port
                                        });
                                        let test_session_reflector =
                                            &mut Session::new(source_address, sender_address);
                                        let session =
                                            session_option.get_or_insert(test_session_reflector);
                                        let udp_socket = session.create_udp_socket()?;
                                        drop(sessions_lock);

                                        let _ = self.worker_event_sender.try_lock()?.send(
                                            EventLoopMessages::Register((
                                                udp_socket,
                                                Box::new(rx_callback(
                                                    source_address,
                                                    self.twamp_sessions.clone(),
                                                )?),
                                            )),
                                        );
                                        let accept_message = AcceptSessionMessage::new(
                                            AcceptFields::Ok,
                                            0,
                                            response.reflector_port,
                                            [0; 16],
                                            [0; 12],
                                            [0; 16],
                                        );
                                        socket.send(accept_message)?;
                                    }
                                    TwampControlCommand::StartNSessions => {
                                        unimplemented!("StartNSessions!");
                                    }
                                    TwampControlCommand::StartNAck => {
                                        unimplemented!("StartNAck!");
                                    }
                                    TwampControlCommand::StopNSessions => {
                                        unimplemented!("StopNSessions!");
                                    }
                                    TwampControlCommand::StopNAck => {
                                        unimplemented!("StopNAck!");
                                    }
                                    _ => {
                                        let accept_message = AcceptSessionMessage::new(
                                            AcceptFields::NotSupported,
                                            0,
                                            response.reflector_port,
                                            [0; 16],
                                            [0; 12],
                                            [0; 16],
                                        );
                                        socket.send(accept_message)?;
                                    }
                                }
                            }
                            Err(_) => {
                                log::error!("Can't parse RequestTwSession bytes");
                                return Err(CommonError::Generic(
                                    "Error parsing RequestTwSession response".to_string(),
                                ));
                            }
                        };
                    } else {
                        log::warn!("Can't receive 0 bytes");
                        return Err(CommonError::Generic("Close signal received".to_string()));
                    }
                }
            }
            ServerCtrlConnectionState::Start => {
                log::info!("Starting");
                // Send start ack message
                let start_ack = ControlMessage {
                    control_command: AcceptFields::Ok as u8,
                    mbz: Default::default(),
                    hmac: Default::default(),
                };
                socket.send(start_ack)?;
                self.state = ServerCtrlConnectionState::Monitor;
                // If any test session completes, do:
                // If it completes successfully,
                // If any test session fails, transition to the error state or retry state
                // depending on the retry and error counts
                // Set a timeout for the TW
            }
            ServerCtrlConnectionState::End => {
                // Send the TWAMP-Stop packet to end each test session
                // If successful, transition to the error state
                // If failed, transition to the error state or retry state
                // depending on the retry and error counts
            }
            ServerCtrlConnectionState::Retry => {
                // Retry the failed step
                // If successful, transition back to the previous state
                // If failed, transition to the error state or retry state
                // depending on the retry and error counts
            }
            ServerCtrlConnectionState::Error => {
                // Handle the error
                // If recoverable, transition back to the previous state
                // If not recoverable, terminate the control connection and stop all test sessions
                log::error!("An error in a transition has occurred");
            }
        }
        Ok(())
    }
}

pub fn rx_callback(
    rx_socket_address: SocketAddr,
    sessions: Arc<RwLock<Vec<Session>>>,
) -> Result<
    impl Fn(&mut TimestampedUdpSocket, network_commons::event_loop::Token) -> Result<isize, CommonError>,
    CommonError,
> {
    Ok(move |inner_socket: &mut TimestampedUdpSocket, _| {
        let buffer = &mut [0; 1 << 16];
        let (result, socket_address, timestamp) = inner_socket.receive_from(buffer)?;
        let (twamp_test_message, _bytes_written): (SenderMessage, usize) =
            SenderMessage::try_from_be_bytes(&buffer[..result.max(0) as usize])?;
        let mut sessions_lock = sessions.write().unwrap();
        let session_option = sessions_lock.iter().find(|session| {
            (session.rx_socket_address == rx_socket_address)
                && (session.tx_socket_address == socket_address)
        });

        if let Some(session) = session_option {
            let reflected_message = ReflectedMessage {
                reflector_sequence_number: session.seq_number.load(Ordering::SeqCst),
                timestamp: NtpTimestamp::from(DateTime::utc_now()),
                error_estimate: ErrorEstimate::new(1, 0, 1, 1),
                mbz1: 0,
                receive_timestamp: NtpTimestamp::from(timestamp),
                sender_sequence_number: twamp_test_message.sequence_number,
                sender_timestamp: twamp_test_message.timestamp,
                sender_error_estimate: twamp_test_message.error_estimate,
                mbz2: 0,
                sender_ttl: 255,
                padding: vec![0_u8; twamp_test_message.padding.len() - MIN_UNAUTH_PADDING],
            };
            inner_socket.send_to(&socket_address, reflected_message.clone())?;
            session.add_to_sent(reflected_message)?;
        } else {
            // Create session
            let session = Session::new(rx_socket_address, socket_address);
            // Create Reflected message
            let reflected_message = ReflectedMessage {
                reflector_sequence_number: session.seq_number.load(Ordering::SeqCst),
                timestamp: NtpTimestamp::from(DateTime::utc_now()),
                error_estimate: ErrorEstimate::new(0, 0, 0, 1),
                mbz1: 0,
                receive_timestamp: NtpTimestamp::from(timestamp),
                sender_sequence_number: twamp_test_message.sequence_number,
                sender_timestamp: twamp_test_message.timestamp,
                sender_error_estimate: twamp_test_message.error_estimate,
                mbz2: 0,
                sender_ttl: 255,
                padding: Vec::new(),
            };
            log::debug!("Reflected message: \n {:?}", reflected_message);
            // Send message
            inner_socket.send_to(&socket_address, reflected_message.clone())?;
            // Add message results to session
            session.add_to_sent(reflected_message)?;
            // Store session
            sessions_lock.push(session);
        }
        Ok(result)
    })
}
