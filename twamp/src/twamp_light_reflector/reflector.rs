#[cfg(target_os = "linux")]
use common::epoll_loop::LinuxEventLoop as EventLoop;

#[cfg(target_os = "macos")]
use common::kevent_loop::MacOSEventLoop as EventLoop;
use message_macro::BeBytes;

use std::{os::fd::IntoRawFd, sync::atomic::Ordering};

use ::common::{error::CommonError, socket::Socket, Strategy, TestResult};
use common::{
    event_loop::EventLoopTrait,
    time::{DateTime, NtpTimestamp},
    udp_socket::{set_timestamping_options, TimestampedUdpSocket},
};

use crate::{
    common::message::{ErrorEstimate, ReflectedMessage, SenderMessage},
    common::{session::Session, MIN_UNAUTH_PADDING},
    twamp_light_sender::result::TwampResult,
};

use super::Configuration;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Reflector {
    configuration: Configuration,
}

impl Reflector {
    pub fn new(configuration: Configuration) -> Self {
        Self { configuration }
    }

    fn create_socket(&mut self) -> Result<TimestampedUdpSocket, CommonError> {
        let socket = mio::net::UdpSocket::bind(self.configuration.source_ip_address.parse()?)?;
        let mut my_socket = TimestampedUdpSocket::new(socket.into_raw_fd());
        #[cfg(target_os = "linux")]
        my_socket.set_socket_options(libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC, None)?;

        #[cfg(target_os = "macos")]
        {
            my_socket.set_socket_options(libc::O_NONBLOCK | libc::O_CLOEXEC, None)?;
        }

        set_timestamping_options(&mut my_socket)?;

        Ok(my_socket)
    }
}

impl Strategy<TwampResult, CommonError> for Reflector {
    fn execute(&mut self) -> std::result::Result<TwampResult, CommonError> {
        // Create the socket
        let my_socket = self.create_socket()?;

        // Creates the event loop with a default socket
        let mut event_loop = EventLoop::new(1024)?;
        let _rx_token = event_loop.register_event_source(my_socket, move |inner_socket, _| {
            let mut sessions: Vec<Session> = Vec::new();
            let buffer = &mut [0; 1 << 16];
            let (result, socket_address, timestamp) = inner_socket.receive_from(buffer)?;
            let (twamp_test_message, _bytes_written): (SenderMessage, usize) =
                SenderMessage::try_from_be_bytes(&buffer[..result])?;

            let session_option = sessions
                .iter()
                .find(|session| session.socket_address == socket_address);

            if let Some(session) = session_option {
                let socket_address = session.socket_address;
                let reflected_message = ReflectedMessage {
                    reflector_sequence_number: session.seq_number.load(Ordering::SeqCst),
                    timestamp: NtpTimestamp::from(DateTime::utc_now()),
                    error_estimate: ErrorEstimate::new(0, 0, 0, 1)?,
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
                session.add_to_sent(Box::new(reflected_message));
            } else {
                // Create session
                let session = Session::from_socket_address(&socket_address);
                // Create Reflected message
                let reflected_message = ReflectedMessage {
                    reflector_sequence_number: session.seq_number.load(Ordering::SeqCst),
                    timestamp: NtpTimestamp::from(DateTime::utc_now()),
                    error_estimate: ErrorEstimate::new(0, 0, 0, 1)?,
                    mbz1: 0,
                    receive_timestamp: NtpTimestamp::from(timestamp),
                    sender_sequence_number: twamp_test_message.sequence_number,
                    sender_timestamp: twamp_test_message.timestamp,
                    sender_error_estimate: twamp_test_message.error_estimate,
                    mbz2: 0,
                    sender_ttl: 255,
                    padding: Vec::new(),
                };
                log::debug!("Refected message: \n {:?}", reflected_message);
                // Send message
                inner_socket.send_to(&socket_address, reflected_message.clone())?;
                // Add message results to session
                session.add_to_sent(Box::new(reflected_message));
                // Store session
                sessions.push(session);
            }
            Ok(result as i32)
        })?;
        // Run the event loop
        event_loop.run()?;

        Ok(TwampResult {
            session_results: Vec::new(),
            error: None,
        })
    }
}

pub struct SessionResult {}

impl TestResult for SessionResult {}
