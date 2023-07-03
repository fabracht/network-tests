#![allow(dead_code)]
use crate::twamp_common::message::ControlMessageType;
use crate::twamp_common::message::ServerGreeting;
use crate::twamp_common::message::ServerStart;
use crate::TwampLight;

use bebytes::BeBytes;
use network_commons::{socket::Socket, tcp_socket::TimestampedTcpSocket, time::NtpTimestamp};

#[derive(BeBytes, Debug)]
struct TestMessage {
    variant: Vec<u8>,
}

// Define the states of the state machine as an enum
enum ControlSessionState {
    Initial,
    Authentication,
    Negotiation,
    Start,
    Monitor,
    End,
    Retry,
    Error,
}

// Define a struct to represent the TWAMP control session
pub struct ControlSession {
    id: i32,
    state: ControlSessionState,
    twamp_sessions: Vec<TwampLight>,
    retry_count: u32, // Number of times to retry failed steps
    error_count: u32, // Number of times to tolerate errors before terminating the session
    auth_timeout: std::time::Duration,
    negotiation_timeout: std::time::Duration,
    start_timeout: std::time::Duration,
    monitor_timeout: std::time::Duration,
}

impl ControlSession {
    // Method to create a new TWAMP control session with the initial state and TCP connection
    pub fn new(token: i32, retry_count: u32, error_count: u32) -> ControlSession {
        ControlSession {
            id: token,
            state: ControlSessionState::Initial,
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
    pub fn transition(&mut self, socket: &mut TimestampedTcpSocket) {
        match self.state {
            ControlSessionState::Initial => {
                let _server_greeting =
                    ServerGreeting::new([3; 12], 3, [1; 16], [1; 16], 3, [10; 12]).unwrap();
                let test_message = TestMessage {
                    variant: b"Hi there".to_vec(),
                };
                log::info!("Sending test message");
                let result = socket.send(test_message);
                match result {
                    // If successful, transition to the authentication state
                    Ok((_result, _)) => {
                        log::info!("Transition to authentication");
                        self.state = ControlSessionState::Authentication
                    }
                    // If failed, transition to the error state or retry state
                    Err(_e) => {
                        log::info!("Transition to error {:?}", _e);

                        self.state = ControlSessionState::Error
                    }
                }
            }
            ControlSessionState::Authentication => {
                log::info!("Authenticating");
                // Authenticate the control connection
                // If successful, transition to the negotiation state
                // If failed, transition to the error state or retry state
                // Set a timeout for the authentication process
                self.state = ControlSessionState::Negotiation;
            }
            ControlSessionState::Negotiation => {
                let _message =
                    ServerStart::new(ControlMessageType::ServerStart, NtpTimestamp::now());

                // Negotiate session parameters
                // If successful, transition to the start state
                // If failed, transition to the retry state
                // Set a timeout for the negotiation process
            }
            ControlSessionState::Start => {
                // Send the TWAMP-Test packet to start each test session
                // If successful, transition to the monitor state
                // If failed, transition to the retry state
                // Set a timeout for the TWAMP-Test packet transmission
                let buffer = &mut [0; 1 << 16];

                let result = socket.receive(buffer);
                if let Ok(result) = result {
                    let _ = socket.send(TestMessage {
                        variant: buffer[..result.0].to_vec(),
                    });
                    log::info!(
                        "Received {} bytes, at {:?} that says {} with token {:?}",
                        result.0,
                        result.1,
                        std::str::from_utf8(buffer).unwrap(),
                        self.id
                    );
                }
            }
            ControlSessionState::Monitor => {
                // Monitor each test session
                // If all test sessions complete successfully, transition to the end state
                // If any test session fails, transition to the error state or retry state
                // depending on the retry and error counts
                // Set a timeout for the TW
            }
            ControlSessionState::End => {
                // Send the TWAMP-Stop packet to end each test session
                // If successful, transition to the error state
                // If failed, transition to the error state or retry state
                // depending on the retry and error counts
            }
            ControlSessionState::Retry => {
                // Retry the failed step
                // If successful, transition back to the previous state
                // If failed, transition to the error state or retry state
                // depending on the retry and error counts
            }
            ControlSessionState::Error => {
                // Handle the error
                // If recoverable, transition back to the previous state
                // If not recoverable, terminate the control connection and stop all test sessions
                log::error!("An error in a transition has occurred");
            }
        }
    }
}
