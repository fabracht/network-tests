#![allow(dead_code)]
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use crate::twamp_common::data_model::TestSessionReflector;
use crate::twamp_common::message::AcceptFields;
use crate::twamp_common::message::AcceptSessionMessage;
use crate::twamp_common::message::ClientSetupResponse;
use crate::twamp_common::message::Modes;
use crate::twamp_common::message::RequestTwSession;
use crate::twamp_common::message::ServerGreeting;
use crate::twamp_common::message::TwampControlCommand;

use bebytes::BeBytes;
use network_commons::epoll_loop::EventLoopMessages;
use network_commons::error::CommonError;
use network_commons::{socket::Socket, tcp_socket::TimestampedTcpSocket};

use super::WorkerSender;
use crate::twamp_common::data_model::ServerCtrlConnectionState;

// Define a struct to represent the TWAMP control session
pub struct ControlSession {
    pub id: i32,
    supported_modes: Modes,
    state: ServerCtrlConnectionState,
    twamp_sessions: Arc<Vec<TestSessionReflector>>,
    retry_count: u32, // Number of times to retry failed steps
    error_count: u32, // Number of times to tolerate errors before terminating the session
    auth_timeout: std::time::Duration,
    negotiation_timeout: std::time::Duration,
    start_timeout: std::time::Duration,
    monitor_timeout: std::time::Duration,
    rx_buffer: [u8; 1 << 16],
    worker_event_sender: Arc<Mutex<WorkerSender>>,
}

impl ControlSession {
    // Method to create a new TWAMP control session with the initial state and TCP connection
    pub fn new(
        token: i32,
        mode: Modes,
        retry_count: u32,
        error_count: u32,
        worker_event_sender: Arc<Mutex<WorkerSender>>,
    ) -> ControlSession {
        ControlSession {
            id: token,
            supported_modes: mode,
            state: ServerCtrlConnectionState::Greeting,
            twamp_sessions: Arc::new(Vec::new()),
            retry_count,
            error_count,
            auth_timeout: std::time::Duration::from_secs(30),
            negotiation_timeout: std::time::Duration::from_secs(30),
            start_timeout: std::time::Duration::from_secs(10),
            monitor_timeout: std::time::Duration::from_secs(10),
            rx_buffer: [0; 1 << 16],
            worker_event_sender,
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
                        log::info!("Transition to Negotiation");
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
            ServerCtrlConnectionState::Negotiation => {
                let result = socket.receive(&mut self.rx_buffer);
                if let Ok(result) = result {
                    if result.0 != 0 {
                        log::info!("Received ClientSetupResponse");
                        match ClientSetupResponse::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                // verify if the mode requested is supported
                                if response.mode & self.supported_modes == response.mode {
                                    self.state = ServerCtrlConnectionState::Authentication;
                                    self.transition(socket)?;
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
            ServerCtrlConnectionState::Authentication => {
                log::info!("Authenticating");

                self.state = ServerCtrlConnectionState::Start;
                self.transition(socket)?;
            }
            ServerCtrlConnectionState::Start => {
                let result = socket.receive(&mut self.rx_buffer);
                log::info!("Received message in Start");
                if let Ok(result) = result {
                    if result.0 != 0 {
                        match RequestTwSession::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                match response.request_type {
                                    TwampControlCommand::Forbidden => {
                                        println!("Unimplemented!");
                                    }
                                    TwampControlCommand::StartSessions => {
                                        // Start sessions
                                        self.state = ServerCtrlConnectionState::Monitor;
                                        self.transition(socket)?;
                                    }
                                    TwampControlCommand::StopSessions => {
                                        println!("Unimplemented!");
                                    }
                                    TwampControlCommand::RequestTwSession => {
                                        log::info!("Received RequestTwSession: {:?}", response);
                                        // Check if port is already in use, if not, propose the next available
                                        let response_ip = response.receiver_address;
                                        let response_port = response.receiver_port;
                                        let source_ip = SocketAddr::V4(SocketAddrV4::new(
                                            Ipv4Addr::new(
                                                response_ip[0],
                                                response_ip[1],
                                                response_ip[2],
                                                response_ip[3],
                                            ),
                                            response_port,
                                        ));
                                        // let ref_wait = response.timeout as u64;
                                        // let session = self
                                        //     .twamp_sessions
                                        //     .iter()
                                        //     .find(|session| {
                                        //         session.configuration.source_ip_address.port()
                                        //             == response.receiver_port
                                        //     })
                                        //     .get_or_insert(&mut TestSessionReflector::new(
                                        //         "sid".to_string(),
                                        //         Configuration::new(&source_ip, ref_wait),
                                        //     ));
                                        // let udp_socket =
                                        //     TestSessionReflector::create_socket(&source_ip)?;
                                        // let sessions = Arc::new(RwLock::new(Vec::new()));
                                        // let callback = rx_callback(source_ip, sessions)?;
                                        // let _ = self.worker_event_sender.lock().unwrap().send(
                                        //     EventLoopMessages::Register((
                                        //         udp_socket,
                                        //         Box::new(callback),
                                        //     )),
                                        // );
                                        let accept_message = AcceptSessionMessage::new(
                                            AcceptFields::Ok,
                                            0,
                                            response.receiver_port,
                                            [0; 16],
                                            [0; 12],
                                            [0; 16],
                                        );
                                        socket.send(accept_message)?;
                                    }
                                    TwampControlCommand::StartNSessions => {
                                        println!("Unimplemented!");
                                    }
                                    TwampControlCommand::StartNAck => {
                                        println!("Unimplemented!");
                                    }
                                    TwampControlCommand::StopNSessions => {
                                        println!("Unimplemented!");
                                    }
                                    TwampControlCommand::StopNAck => {
                                        println!("Unimplemented!");
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
            ServerCtrlConnectionState::Monitor => {
                log::info!("Monitoring");
                // Monitor each test session

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
