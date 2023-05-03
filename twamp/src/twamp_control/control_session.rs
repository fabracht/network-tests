use common::{
    error::CommonError, socket::Socket, tcp_socket::TimestampedTcpSocket, time::NtpTimestamp,
    Strategy,
};

use crate::{
    common::{ControlMessageType, ServerGreeting, ServerStart},
    twamp_light_sender::{result::TwampResult, twamp_light::TwampLight},
};

// Define the states of the state machine as an enum
enum ControlSessionState<'a> {
    Initial(&'a mut TimestampedTcpSocket),
    Authentication(&'a mut TimestampedTcpSocket),
    Negotiation(&'a mut TimestampedTcpSocket),
    Start(&'a mut TimestampedTcpSocket),
    Monitor(&'a mut TimestampedTcpSocket),
    End(&'a mut TimestampedTcpSocket),
    Retry(&'a mut TimestampedTcpSocket),
    Error(&'a mut TimestampedTcpSocket),
}

// Define a struct to represent the TWAMP control session
pub struct ControlSession<'a> {
    state: ControlSessionState<'a>,
    twamp_sessions: Vec<TwampLight>,
    retry_count: u32, // Number of times to retry failed steps
    error_count: u32, // Number of times to tolerate errors before terminating the session
    auth_timeout: std::time::Duration,
    negotiation_timeout: std::time::Duration,
    start_timeout: std::time::Duration,
    monitor_timeout: std::time::Duration,
}

impl<'a> ControlSession<'a> {
    // Method to create a new TWAMP control session with the initial state and TCP connection
    pub fn new(
        tcp_stream: &mut TimestampedTcpSocket,
        retry_count: u32,
        error_count: u32,
    ) -> ControlSession {
        ControlSession {
            state: ControlSessionState::Initial(tcp_stream),
            twamp_sessions: Vec::new(),
            retry_count,
            error_count,
            auth_timeout: std::time::Duration::from_secs(30),
            negotiation_timeout: std::time::Duration::from_secs(30),
            start_timeout: std::time::Duration::from_secs(10),
            monitor_timeout: std::time::Duration::from_secs(10),
        }
    }

    // Method to add a new TWAMP test session to the control session
    fn add_twamp_session(&mut self, twamp_session: TwampLight) {
        self.twamp_sessions.push(twamp_session);
    }

    // Method to transition to the next state of the state machine
    fn transition(mut self) {
        match self.state {
            ControlSessionState::Initial(socket) => {
                // Start the control connection
                let server_greeting = ServerGreeting::default();
                let result = socket.send(server_greeting);
                match result {
                    // If successful, transition to the authentication state
                    Ok((result, _)) => self.state = ControlSessionState::Authentication(socket),
                    // If failed, transition to the error state or retry state
                    Err(_e) => self.state = ControlSessionState::Error(socket),
                }
            }
            ControlSessionState::Authentication(socket) => {
                // Authenticate the control connection
                // If successful, transition to the negotiation state
                // If failed, transition to the error state or retry state
                // Set a timeout for the authentication process
                self.state = ControlSessionState::Negotiation(socket);
            }
            ControlSessionState::Negotiation(socket) => {
                let message =
                    ServerStart::new(ControlMessageType::ServerStart, NtpTimestamp::now());
                // Negotiate session parameters
                // If successful, transition to the start state
                // If failed, transition to the retry state
                // Set a timeout for the negotiation process
            }
            ControlSessionState::Start(socket) => {
                // Send the TWAMP-Test packet to start each test session
                // If successful, transition to the monitor state
                // If failed, transition to the retry state
                // Set a timeout for the TWAMP-Test packet transmission
            }
            ControlSessionState::Monitor(socket) => {
                // Monitor each test session
                // If all test sessions complete successfully, transition to the end state
                // If any test session fails, transition to the error state or retry state
                // depending on the retry and error counts
                // Set a timeout for the TW
            }
            ControlSessionState::End(socket) => {
                // Send the TWAMP-Stop packet to end each test session
                // If successful, transition to the error state
                // If failed, transition to the error state or retry state
                // depending on the retry and error counts
            }
            ControlSessionState::Retry(socket) => {
                // Retry the failed step
                // If successful, transition back to the previous state
                // If failed, transition to the error state or retry state
                // depending on the retry and error counts
            }
            ControlSessionState::Error(socket) => {
                // Handle the error
                // If recoverable, transition back to the previous state
                // If not recoverable, terminate the control connection and stop all test sessions
            }
        }
    }
}