use crate::twamp_common::data_model::AcceptFields;
use crate::twamp_common::data_model::Modes;
use crate::twamp_common::data_model::SenderSessionState;
use crate::twamp_common::data_model::TwampControlCommand;
use crate::twamp_common::message::ControlMessage;
use crate::twamp_common::message::RequestTwSessionBuilder;
use crate::twamp_common::message::ServerGreeting;
use crate::twamp_common::message::ServerStart;
use crate::twamp_common::message::{AcceptSessionMessage, ClientSetupResponse};
use crate::twamp_common::session::Session;
use crate::twamp_light_sender::twamp_light::create_rx_callback;
use crate::twamp_light_sender::twamp_light::create_tx_callback;
use crate::twamp_light_sender::twamp_light::SessionSender;
use crate::twamp_light_sender::Configuration;
use bebytes::BeBytes;
use network_commons::epoll_loop::DuplexChannel;
use network_commons::epoll_loop::EventLoopMessages;
use network_commons::error::CommonError;
use network_commons::event_loop::Itimerspec;
use network_commons::event_loop::Token;
use network_commons::time::NtpTimestamp;
use network_commons::udp_socket::TimestampedUdpSocket;
use network_commons::{socket::Socket, tcp_socket::TimestampedTcpSocket};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;

#[allow(unused)]
// Define a struct to represent the TWAMP control session
pub struct ClientControlSession {
    auth_timeout: std::time::Duration,
    pub id: i32,
    negotiation_timeout: std::time::Duration,
    rc_sessions: Arc<RwLock<Vec<Session>>>,
    retry_count: u32,
    rx_buffer: [u8; 1 << 16],
    start_timeout: std::time::Duration,
    state: SenderSessionState,
    supported_modes: Modes,
    test_session: SessionSender,
    worker_event_sender: Arc<Mutex<DuplexChannel<TimestampedUdpSocket>>>,
}

impl ClientControlSession {
    // Method to create a new TWAMP control session with the initial state and TCP connection
    pub fn new(
        token: i32,
        mode: Modes,
        rc_sessions: Arc<RwLock<Vec<Session>>>,
        retry_count: u32,
        sessions_configuration: Configuration,
        worker_event_sender: Arc<Mutex<DuplexChannel<TimestampedUdpSocket>>>,
    ) -> ClientControlSession {
        ClientControlSession {
            id: token,
            supported_modes: mode,
            state: SenderSessionState::AwaitingServerGreeting,
            test_session: SessionSender::new(&sessions_configuration),
            rc_sessions,
            retry_count,
            auth_timeout: std::time::Duration::from_secs(30),
            negotiation_timeout: std::time::Duration::from_secs(30),
            start_timeout: std::time::Duration::from_secs(10),
            rx_buffer: [0; 1 << 16],
            worker_event_sender,
        }
    }

    // Method to transition to the next state of the state machine
    pub fn transition(&mut self, socket: &mut TimestampedTcpSocket) -> Result<(), CommonError> {
        match self.state {
            SenderSessionState::AwaitingServerGreeting => {
                let result = socket.receive(&mut self.rx_buffer);
                if let Ok(result) = result {
                    if result.0 != 0 {
                        log::info!("Received Server Greeting");
                        match ServerGreeting::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                // verify if the mode requested is supported
                                if response.modes & self.supported_modes == response.modes {
                                    self.state = SenderSessionState::SendingClientSetup;
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
                        }
                    }
                };
            }
            SenderSessionState::SendingClientSetup => {
                let client_setup =
                    ClientSetupResponse::new(self.supported_modes, [0u8; 80], [0u8; 64], [0u8; 16]);
                let result = socket.send(client_setup);
                match result {
                    // If successful, transition to the authentication state
                    Ok((_result, _)) => {
                        log::info!("Transition to AwaitingServerStart");
                        self.state = SenderSessionState::AwaitingServerStart
                    }
                    // If failed, transition to the error state or retry state
                    Err(_e) => {
                        return Err(CommonError::Generic("Error SendingClientSetup".to_string()));
                    }
                }
            }
            SenderSessionState::AwaitingServerStart => {
                let result = socket.receive(&mut self.rx_buffer);
                if let Ok(result) = result {
                    if result.0 != 0 {
                        log::info!("Received Server Start");
                        match ServerStart::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                match response.accept {
                                    AcceptFields::NotSupported => {
                                        log::error!("Not supported");
                                        return Err(CommonError::Generic(
                                            "Accept field value NotSupported".to_string(),
                                        ));
                                    }
                                    AcceptFields::Ok => {
                                        self.state = SenderSessionState::SendingRequestSession;
                                        self.transition(socket)?;
                                    }
                                    AcceptFields::Failure => todo!(),
                                    AcceptFields::InternalError => todo!(),
                                    AcceptFields::PermanentResourceLimitation => todo!(),
                                    AcceptFields::TemporaryResourceLimitation => todo!(),
                                }
                                ///////////////////////
                            }
                            Err(_) => {
                                log::error!("Can't parse Start bytes");
                                return Err(CommonError::Generic(
                                    "Error parsing Start response".to_string(),
                                ));
                            }
                        }
                    }
                };
            }
            SenderSessionState::SendingRequestSession => {
                // done with the connection setup process, ready to request test sessions
                let ipvn = match self.test_session.source_ip_address {
                    std::net::SocketAddr::V4(_) => 4,
                    std::net::SocketAddr::V6(_) => 6,
                };
                let padding = self.test_session.padding;
                let timeout = self.test_session.last_message_timeout;
                let sender_port = self.test_session.source_ip_address.port();
                let sender_ip = self.test_session.source_ip_address.ip();
                let receiver_address = self.test_session.targets.first().unwrap();
                let request_tw_session_builder = RequestTwSessionBuilder::new()
                    .request_type(TwampControlCommand::RequestTwSession)
                    .ipvn(ipvn)
                    .num_schedule_slots(0)
                    .num_packets(0)
                    .sender_port(sender_port)
                    .receiver_port(receiver_address.port())
                    .sender_address(sender_ip)
                    .receiver_address(receiver_address.ip())
                    .sid([0u8; 16])
                    .padding_length(padding as u32)
                    .start_time(NtpTimestamp::now())
                    .timeout(timeout.as_secs() as u32)
                    .type_p(0)
                    .hmac([0u8; 16]);
                let request_tw_session = request_tw_session_builder.build()?;

                let result = socket.send(request_tw_session);
                match result {
                    // If successful, transition into Monitor state
                    Ok((_result, _)) => {
                        log::info!("Transition to AwaitingSessionAcceptance");
                        self.state = SenderSessionState::AwaitingSessionAcceptance
                    }
                    // If failed, transition to the error state or retry state
                    Err(_e) => {
                        return Err(CommonError::Generic(
                            "Error sending RequestTwSession response".to_string(),
                        ));
                    }
                };
            }
            SenderSessionState::AwaitingSessionAcceptance => {
                log::info!("Monitoring");
                // Here we monitor for AcceptSessionMessages. For every Tw schedule we should MUST receive an AcceptSessionMessage.
                let result = socket.receive(&mut self.rx_buffer);
                if let Ok(result) = result {
                    if result.0 != 0 {
                        log::info!("Received AwaitingSessionAcceptance Message");
                        match AcceptSessionMessage::try_from_be_bytes(&self.rx_buffer) {
                            Ok((response, _bytes_written)) => {
                                if response.accept == AcceptFields::Ok {
                                    log::info!("Transition to SessionEstablished");
                                    self.state = SenderSessionState::SessionEstablished;
                                } else {
                                    self.state = SenderSessionState::SessionRefused;
                                }
                                self.transition(socket)?;
                            }
                            Err(_) => {
                                log::error!("Can't parse Accept bytes");
                                return Err(CommonError::Generic(
                                    "Error parsing Greeting response".to_string(),
                                ));
                            }
                        }
                    }
                };
            }
            SenderSessionState::SessionEstablished => {
                let start_command = ControlMessage {
                    control_command: TwampControlCommand::StartSessions as u8,
                    mbz: Default::default(),
                    hmac: Default::default(),
                };
                socket.send(start_command)?;
                self.state = SenderSessionState::AwaitingStartAck;
                log::info!("Transition to AwaitingStartAck");
            }
            SenderSessionState::AwaitingStartAck => {
                let result = socket.receive(&mut self.rx_buffer)?;
                if result.0 != 0 {
                    match ControlMessage::try_from_be_bytes(&self.rx_buffer) {
                        Ok((response, _bytes_written)) => {
                            if response.control_command == AcceptFields::Ok as u8 {
                                // Server has accepted the start command, so we can start streaming test messages
                                log::info!("Received Ack, start streaming");
                                self.state = SenderSessionState::TestInProgress;
                                self.transition(socket)?;
                            }
                        }
                        Err(_) => {
                            log::error!("Can't parse Accept bytes");
                            self.state = SenderSessionState::FinalState;
                        }
                    }
                }
            }
            SenderSessionState::TestInProgress => {
                // We can now start the test sessions

                let session_socket = self.test_session.create_udp_socket()?;

                let rx_message = EventLoopMessages::Register((
                    session_socket,
                    Box::new(create_rx_callback(self.rc_sessions.clone()))
                        as Box<
                            dyn FnMut(
                                    &mut TimestampedUdpSocket,
                                    Token,
                                ) -> Result<isize, CommonError>
                                + Send,
                        >,
                ));

                // This configures the tx socket timer.
                let timer_spec = Itimerspec {
                    it_interval: self.test_session.packet_interval,
                    it_value: Duration::from_millis(10),
                };
                let sender_lock = self.worker_event_sender.try_lock()?;
                sender_lock.send(rx_message)?;
                drop(sender_lock);
                log::info!("Register Message sent");
                loop {
                    std::thread::sleep(Duration::from_millis(100));
                    log::info!("Slept");
                    let sender_lock = self.worker_event_sender.try_lock()?;
                    if let Ok(token) = sender_lock.get_token() {
                        let tx_message = EventLoopMessages::RegisterTimed((
                            timer_spec,
                            token,
                            Box::new(create_tx_callback(
                                self.rc_sessions.clone(),
                                self.test_session.padding,
                            ))
                                as Box<
                                    dyn FnMut(
                                            &mut TimestampedUdpSocket,
                                            Token,
                                        )
                                            -> Result<isize, CommonError>
                                        + Send,
                                >,
                        ));
                        sender_lock.send(tx_message)?;
                        log::info!("Registered callbacks");
                        break;
                    }
                    continue;
                }
                let timeout = self.test_session.duration + self.test_session.last_message_timeout;

                let timer_spec = Itimerspec {
                    it_interval: Duration::from_millis(10),
                    it_value: timeout,
                };

                let sender_lock = self.worker_event_sender.try_lock()?;

                let thread = std::thread::current();
                let tx_message = EventLoopMessages::TimedCleanup { timer_spec, thread };
                sender_lock.send(tx_message)?;
                drop(sender_lock);
                std::thread::park();
                std::thread::sleep(timeout);

                log::info!("Waiting for cleanup");

                let stop_sessions = ControlMessage {
                    control_command: TwampControlCommand::StopSessions as u8,
                    mbz: Default::default(),
                    hmac: Default::default(),
                };
                socket.send(stop_sessions)?;
                return Ok(());
            }
            SenderSessionState::SessionRefused => {
                return Err(CommonError::Generic("SessionRefused Error".to_string()));
            }
            SenderSessionState::ClosingConnection => {
                return Err(CommonError::Generic("ClosingConnection Error".to_string()));
            }
            SenderSessionState::FinalState => {
                log::info!(
                    "This is just temporary, there is no actual final state defined in the RFC"
                );
                return Ok(());
            }
        }
        Ok(())
    }
}
