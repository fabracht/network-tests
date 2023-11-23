use libc::close;
#[cfg(target_os = "linux")]
use network_commons::epoll_loop::LinuxEventLoop as EventLoop;
use std::{
    os::fd::{AsRawFd, IntoRawFd},
    sync::{Arc, Mutex, RwLock},
};

use network_commons::{
    epoll_loop::EventLoopMessages, error::CommonError, event_loop::EventLoopTrait, socket::Socket,
    tcp_socket::TimestampedTcpSocket, udp_socket::TimestampedUdpSocket, Strategy,
};

use crate::{
    twamp_common::message::{Mode, Modes},
    twamp_light_sender::result::TwampResult,
};

use super::{control_session::ControlSession, ControlConfiguration};

pub struct Control {
    configuration: ControlConfiguration,
    control_sessions: Arc<RwLock<Vec<ControlSession>>>,
}

impl Control {
    pub fn new(configuration: ControlConfiguration) -> Self {
        Self {
            configuration,
            control_sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Strategy<TwampResult, CommonError> for Control {
    fn execute(&mut self) -> std::result::Result<TwampResult, CommonError> {
        // std::thread::scope(|scp| {
        let (tx, rx) = std::sync::mpsc::channel();
        let _thread_handle = std::thread::spawn(move || -> std::result::Result<(), CommonError> {
            let mut event_loop: EventLoop<TimestampedUdpSocket> = EventLoop::new(1024).unwrap();
            let event_sender = event_loop.get_communication_channel();
            tx.send(event_sender).unwrap();
            event_loop.run()?;
            Ok(())
        });
        // Get event sender from worker thread event loop
        let worker_event_sender = rx.recv().unwrap();

        // Create the TcpSocket
        let addr = self.configuration.source_ip_address;
        let listener = mio::net::TcpListener::bind(addr)?;

        let mut socket = TimestampedTcpSocket::new(listener.into_raw_fd());
        log::info!("Created tcp socket");

        #[cfg(target_os = "linux")]
        socket.set_fcntl_options()?;
        log::info!("Set socket options");
        socket.set_timestamping_options()?;

        socket.listen(0)?;
        // Create the event loop
        let mut event_loop = EventLoop::new(1024)?;

        let event_sender = event_loop.get_communication_channel();
        // Register the socket
        let control_sessions = self.control_sessions.clone();
        // Accept incoming connections
        let _register_result = event_loop.register_event_source(
            socket,
            Box::new(move |listener: &mut TimestampedTcpSocket, token| {
                let event_sender = event_sender.clone();
                let (mut timestamped_socket, socket_address) = listener.accept()?;
                let timestamped_socket_raw_fd = timestamped_socket.as_raw_fd();
                let wes = Arc::new(Mutex::new(worker_event_sender.clone()));
                let unauthenticated = Mode::Unauthenticated;
                let authenticated = Mode::Authenticated;
                let mut modes = Modes::new(0);
                modes.set(unauthenticated);
                modes.set(authenticated);

                let mut control_session =
                    ControlSession::new(timestamped_socket_raw_fd, modes, 1, 1, wes);
                log::info!("Accepted connection from {}", socket_address);
                log::info!("Internal token: {:?}", token);

                control_session.transition(&mut timestamped_socket)?;
                control_sessions.write().unwrap().push(control_session);
                let arc_sessions = Arc::clone(&control_sessions);
                let _ = event_sender.send(EventLoopMessages::Register((
                    timestamped_socket,
                    Box::new(move |socket, _token| {
                        let mut cs_lock = arc_sessions.try_write().unwrap();
                        let control_session_entry = cs_lock
                            .iter_mut()
                            .find(|session| &session.id == &socket.as_raw_fd());
                        if let Some(cs) = control_session_entry {
                            if let Err(e) = cs.transition(socket) {
                                log::info!("Closing control socket, {}", e);
                                unsafe { close(socket.as_raw_fd()) };
                            }
                        }

                        Ok(0)
                    }),
                )));

                Ok(0)
            }),
        )?;

        event_loop.run()?;
        Ok(TwampResult {
            session_results: Vec::new(),
            error: None,
        })
    }
}
