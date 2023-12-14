use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use network_commons::{
    epoll_loop::LinuxEventLoop as EventLoop,
    error::CommonError,
    event_loop::{EventLoopTrait, Itimerspec},
    socket::Socket,
    tcp_socket::TimestampedTcpSocket,
    udp_socket::TimestampedUdpSocket,
    Strategy,
};

use crate::{
    twamp_common::{
        data_model::{Mode, Modes},
        session::Session,
    },
    twamp_control::control_client_session::ClientControlSession,
    twamp_light_sender::{
        twamp_light::calculate_session_results, Configuration as TestSessionsConfiguration,
    },
    TwampResult,
};

use super::ClientConfiguration;

/// The control client.
#[derive(Debug)]
pub struct ControlClient {
    /// The control connections of the control client.
    control_configuration: ClientConfiguration,
    test_sessions_configuration: TestSessionsConfiguration,
}

impl ControlClient {
    pub fn new(
        configuration: &ClientConfiguration,
        test_sessions_configuration: &TestSessionsConfiguration,
    ) -> Self {
        log::info!("Configuration: {:?}", configuration);

        log::info!("Created control client, {:?}", configuration);
        Self {
            control_configuration: configuration.to_owned(),
            test_sessions_configuration: test_sessions_configuration.to_owned(),
        }
    }
}

impl Strategy<TwampResult, CommonError> for ControlClient {
    fn execute(&mut self) -> Result<TwampResult, CommonError> {
        log::info!("Executing control client");
        let (tx, rx) = std::sync::mpsc::channel();
        let overtime = Duration::from_secs(self.test_sessions_configuration.last_message_timeout);
        let duration = Duration::from_secs(self.test_sessions_configuration.duration);
        let sessions_handle =
            std::thread::spawn(move || -> std::result::Result<(), CommonError> {
                let mut event_loop: EventLoop<TimestampedUdpSocket> = EventLoop::new(1024)?;
                event_loop.set_overtime(Itimerspec {
                    it_interval: Duration::ZERO,
                    it_value: overtime,
                });

                let event_sender = event_loop.get_communication_channel();
                tx.send(event_sender)?;
                event_loop.run()?;
                Ok(())
            });
        let sessions_configuration = self.test_sessions_configuration.to_owned();
        let control_host = self.control_configuration.control_host;
        let socket_addr = self.control_configuration.source_address;

        // let _control_handle = std::thread::spawn(move || -> Result<(), CommonError> {
        // Get event sender from worker thread event loop
        let worker_event_sender = rx.recv().unwrap();
        let wes = worker_event_sender;

        // ///////////////////////////////////////////////////////////////////////////////
        // // Temporary setup using just 1 source socket and 1 control server connection
        // let ctrl_connection = self.configuration;
        // let source_address = ctrl_connection.client_socket_addr;
        // let dest_address = ctrl_connection.server_socket_address;
        /////////////////////////////////////////////////
        let mut socket = TimestampedTcpSocket::bind(&socket_addr)?;

        log::warn!("Connecting to {:?}", control_host);
        socket.connect(control_host)?;

        let mut control_event_loop = EventLoop::new(1024)?;
        let sessions = sessions_configuration
            .hosts
            .iter()
            .map(|host| Session::new(sessions_configuration.source_ip_address, *host))
            .collect::<Vec<Session>>();
        let rc_sessions = Arc::new(RwLock::new(sessions));

        let mut client_control_session = ClientControlSession::new(
            0,
            Modes::new(Mode::Unauthenticated.into()),
            rc_sessions.clone(),
            0,
            sessions_configuration,
            wes,
        );
        log::info!("Created tcp socket");

        #[cfg(target_os = "linux")]
        socket.set_fcntl_options()?;
        log::info!("Set socket options");
        socket.set_timestamping_options()?;

        let _event_sender = control_event_loop.get_communication_channel();
        let _register_result = control_event_loop.register_event_source(
            socket,
            Box::new(move |listener: &mut TimestampedTcpSocket, _token| {
                client_control_session.transition(listener)?;
                Ok(0)
            }),
        )?;
        control_event_loop.set_overtime(Itimerspec {
            it_interval: Duration::ZERO,
            it_value: overtime,
        });
        control_event_loop.add_duration(&Itimerspec {
            it_interval: Duration::ZERO,
            it_value: duration + overtime,
        })?;
        control_event_loop.run()?;
        let _ = sessions_handle.join();
        let session_results = calculate_session_results(rc_sessions)?;
        Ok(TwampResult {
            session_results,
            error: None,
        })
    }
}
