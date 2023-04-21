#[cfg(target_os = "linux")]
use common::epoll_loop::LinuxEventLoop as EventLoop;
use std::os::fd::IntoRawFd;

use common::{
    error::CommonError, event_loop::EventLoopTrait, tcp_socket::TimestampedTcpSocket, Strategy,
};

use crate::twamp_light_sender::result::TwampResult;

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

impl Strategy<TwampResult, CommonError> for Control {
    fn execute(&mut self) -> std::result::Result<TwampResult, CommonError> {
        // Create the TcpSocket
        let listener = mio::net::TcpListener::bind(self.configuration.source_ip_address.parse()?)?;
        let mut control_sessions: Vec<ControlSession> = Vec::new();

        let mut socket = TimestampedTcpSocket::new(listener.into_raw_fd());
        #[cfg(target_os = "linux")]
        socket.set_socket_options(libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC, None)?;

        let value = libc::SOF_TIMESTAMPING_SOFTWARE
            | libc::SOF_TIMESTAMPING_RX_SOFTWARE
            | libc::SOF_TIMESTAMPING_TX_SOFTWARE;
        socket.set_socket_options(libc::SO_TIMESTAMPING, Some(value as i32))?;

        // Create the event loop
        let mut event_loop = EventLoop::new(1024);

        // Register the socket
        let accept_token = event_loop.register_event_source(socket, move |socket| {
            let (_timestamped_socket, socket_address) = socket.accept()?;
            log::info!("Accepted connection from {}", socket_address);
            let control_session = ControlSession::new(_timestamped_socket, 1, 1);
            control_sessions.push(control_session);
            Ok(0)
        })?;

        event_loop.run()?;
        Ok(TwampResult {
            session_results: Vec::new(),
            error: None,
        })
    }
}
