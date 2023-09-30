// #[cfg(target_os = "linux")]
// use network_commons::epoll_loop::LinuxEventLoop as EventLoop;
// use std::{
//     cell::RefCell,
//     collections::HashMap,
//     os::fd::{AsRawFd, IntoRawFd},
//     rc::Rc,
// };

// use network_commons::{
//     error::CommonError, event_loop::EventLoopTrait, socket::Socket,
//     tcp_socket::TimestampedTcpSocket, Strategy,
// };

// use crate::twamp_light_sender::result::TwampResult;

// use super::{control_session::ControlSession, Configuration};

// pub struct Control {
//     configuration: Configuration,
//     control_sessions: Rc<RefCell<HashMap<i32, ControlSession>>>,
// }

// impl Control {
//     pub fn new(configuration: Configuration) -> Self {
//         Self {
//             configuration,
//             control_sessions: Rc::new(RefCell::new(HashMap::new())),
//         }
//     }
// }

// impl Strategy<TwampResult, CommonError> for Control {
//     fn execute(&mut self) -> std::result::Result<TwampResult, CommonError> {
//         // Create the TcpSocket
//         let listener = mio::net::TcpListener::bind(self.configuration.source_ip_address.parse()?)?;

//         let mut socket = TimestampedTcpSocket::new(listener.into_raw_fd());
//         #[cfg(target_os = "linux")]
//         socket.set_socket_options(
//             libc::SOL_SOCKET,
//             libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
//             None,
//         )?;

//         // let value = libc::SOF_TIMESTAMPING_SOFTWARE
//         //     | libc::SOF_TIMESTAMPING_RX_SOFTWARE
//         //     | libc::SOF_TIMESTAMPING_TX_SOFTWARE;
//         // socket.set_socket_options(libc::SO_TIMESTAMPING, Some(value as i32))?;
//         socket.set_timestamping_options()?;

//         socket.listen(0)?;
//         // Create the event loop
//         let mut event_loop = EventLoop::new(1024)?;

//         let event_sender = event_loop.get_communication_channel();
//         // Register the socket
//         let control_sessions = self.control_sessions.clone();

//         // Accept incoming connections
//         let accept_token = event_loop.register_event_source(socket, move |listener, token| {
//             let control_sessions = control_sessions.clone();
//             let (mut timestamped_socket, socket_address) = listener.accept()?;
//             log::info!("Accepted connection from {}", socket_address);
//             log::info!("Internal token: {:?}", token);
//             let mut control_session = ControlSession::new(1, 1, 1);
//             control_session.transition(&mut timestamped_socket);

//             let _cs = control_sessions
//                 .borrow_mut()
//                 .entry(timestamped_socket.as_raw_fd())
//                 .or_insert(control_session);
//             // Register client socket
//             let _ = event_sender.send((
//                 timestamped_socket,
//                 Box::new(move |socket, _token| {
//                     log::info!("Received a message");
//                     let mut borrowed = control_sessions.borrow_mut();
//                     let control_session_entry = borrowed.get_mut(&socket.as_raw_fd());
//                     if let Some(cs) = control_session_entry {
//                         log::info!("Transitioning");
//                         cs.transition(socket);
//                     }

//                     Ok(0)
//                 }),
//             ));
//             // let accepted_token = token_rx.try_recv();

//             Ok(0)
//         })?;
//         log::warn!("Registered new tcp socket with token {:?}", accept_token);

//         event_loop.run()?;
//         Ok(TwampResult {
//             session_results: Vec::new(),
//             error: None,
//         })
//     }
// }
