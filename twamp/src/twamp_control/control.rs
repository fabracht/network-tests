#[cfg(target_os = "linux")]
use common::epoll_loop::LinuxEventLoop as EventLoop;
use message_macro::BeBytes;
use std::os::fd::{AsRawFd, IntoRawFd};

use common::{
    error::CommonError, event_loop::EventLoopTrait, socket::Socket,
    tcp_socket::TimestampedTcpSocket, Strategy,
};

use crate::{common::ServerGreeting, twamp_light_sender::result::TwampResult};

use super::{control_session::ControlSession, Configuration};

pub struct Control {
    configuration: Configuration,
    control_sessions: Vec<ControlSession>,
}

impl Control {
    pub fn new(configuration: Configuration) -> Self {
        Self {
            configuration,
            control_sessions: Vec::new(),
        }
    }
}

#[derive(BeBytes, Debug)]
struct TestMessage {
    variant: Vec<u8>,
}

impl Strategy<TwampResult, CommonError> for Control {
    fn execute(&mut self) -> std::result::Result<TwampResult, CommonError> {
        // Create the TcpSocket
        let listener = mio::net::TcpListener::bind(self.configuration.source_ip_address.parse()?)?;

        let mut socket = TimestampedTcpSocket::new(listener.into_raw_fd());
        #[cfg(target_os = "linux")]
        socket.set_socket_options(libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC, None)?;

        let value = libc::SOF_TIMESTAMPING_SOFTWARE
            | libc::SOF_TIMESTAMPING_RX_SOFTWARE
            | libc::SOF_TIMESTAMPING_TX_SOFTWARE;
        socket.set_socket_options(libc::SO_TIMESTAMPING, Some(value as i32))?;
        socket.listen(0)?;
        // Create the event loop
        let mut event_loop = EventLoop::new(1024)?;

        let event_sender = event_loop.get_communication_channel();
        // Register the socket

        let _accept_token = event_loop.register_event_source(socket, move |socket| {
            let (_timestamped_socket, socket_address) = socket.accept()?;
            let server_greeting = ServerGreeting::new([3; 12], 3, [1; 16], [1; 16], 3, [10; 12])?;
            let test_message = TestMessage {
                variant: "Hello".to_string().into_bytes(),
            };
            log::info!("Sending server greeting");
            _timestamped_socket.send(server_greeting)?;
            // log::info!("Sending test message");
            // _timestamped_socket.send(test_message)?;

            log::info!("Accepted connection from {}", socket_address);
            let _ = event_sender.send((
                _timestamped_socket,
                Box::new(move |socket| {
                    let buffer = &mut [0; 1 << 16];

                    let result = socket.receive(buffer)?;
                    log::info!(
                        "Received {} bytes, at {:?} that says {}",
                        result.0,
                        result.1,
                        std::str::from_utf8(buffer).unwrap()
                    );
                    Ok(0)
                }),
            ));
            log::warn!("Sent message to received socket");

            log::info!("Sent fd ");
            Ok(0)
        })?;

        event_loop.run()?;
        Ok(TwampResult {
            session_results: Vec::new(),
            error: None,
        })
    }
}
