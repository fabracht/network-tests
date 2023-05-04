#[cfg(target_os = "linux")]
use common::epoll_loop::LinuxEventLoop as EventLoop;
use message_macro::BeBytes;
use std::{cell::RefCell, os::fd::IntoRawFd, rc::Rc};

use common::{
    error::CommonError, event_loop::EventLoopTrait, socket::Socket,
    tcp_socket::TimestampedTcpSocket, Strategy,
};

use crate::twamp_light_sender::result::TwampResult;

use super::{control_session::ControlSession, Configuration};

pub struct Control {
    configuration: Configuration,
    control_sessions: Rc<RefCell<Vec<ControlSession>>>,
}

impl Control {
    pub fn new(configuration: Configuration) -> Self {
        Self {
            configuration,
            control_sessions: Rc::new(RefCell::new(Vec::new())),
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
        let control_sessions = self.control_sessions.clone();

        // Accept incoming connections
        let accept_token = event_loop.register_event_source(socket, move |socket, token| {
            let cs = control_sessions.clone();
            let (mut timestamped_socket, socket_address) = socket.accept()?;
            log::info!("Accepted connection from {}", socket_address);
            log::info!("Internal token: {:?}", token);
            let mut control_session = ControlSession::new(1, 1);
            control_session.transition(&mut timestamped_socket);
            cs.borrow_mut().push(control_session);

            // Register client socket
            let _ = event_sender.send((
                timestamped_socket,
                Box::new(move |socket, token| {
                    let buffer = &mut [0; 1 << 16];

                    let result = socket.receive(buffer)?;
                    socket.send(TestMessage {
                        variant: buffer[..result.0].to_vec(),
                    })?;
                    log::error!(
                        "Received {} bytes, at {:?} that says {} with token {:?}",
                        result.0,
                        result.1,
                        std::str::from_utf8(buffer).unwrap(),
                        token
                    );
                    Ok(0)
                }),
            ));
            // let accepted_token = token_rx.try_recv();

            Ok(0)
        })?;
        log::warn!("Registered new tcp socket with token {:?}", accept_token);

        event_loop.run()?;
        Ok(TwampResult {
            session_results: Vec::new(),
            error: None,
        })
    }
}
