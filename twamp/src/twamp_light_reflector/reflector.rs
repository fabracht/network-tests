use crate::twamp_common::data_model::ErrorEstimate;
use crate::twamp_common::message::ReflectedMessage;
use crate::twamp_common::session::Session;
use crate::twamp_common::MIN_UNAUTH_PADDING;
#[cfg(target_os = "linux")]
use network_commons::epoll_loop::LinuxEventLoop as EventLoop;

use bebytes::BeBytes;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;
use std::{os::fd::IntoRawFd, sync::atomic::Ordering, time::Duration};

use network_commons::{error::CommonError, socket::Socket, Strategy, TestResult};
use network_commons::{
    event_loop::{EventLoopTrait, Itimerspec},
    time::{DateTime, NtpTimestamp},
    udp_socket::TimestampedUdpSocket,
};

use crate::{twamp_common::message::SenderMessage, twamp_light_sender::result::TwampResult};

use super::Configuration;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Reflector {
    pub configuration: Configuration,
}

impl Reflector {
    pub fn new(configuration: Configuration) -> Self {
        Self { configuration }
    }

    pub fn create_socket(&mut self) -> Result<TimestampedUdpSocket, CommonError> {
        let socket = mio::net::UdpSocket::bind(self.configuration.source_ip_address)?;
        let mut my_socket = TimestampedUdpSocket::new(socket.into_raw_fd());
        my_socket.set_fcntl_options()?;
        my_socket.set_timestamping_options()?;
        my_socket.set_socket_options(libc::SOL_IP, libc::IP_RECVERR, Some(1))?;
        my_socket.set_socket_options(libc::IPPROTO_IP, libc::IP_RECVTOS, Some(1))?;

        Ok(my_socket)
    }

    pub fn create_session(
        &mut self,
        event_loop: &mut EventLoop<TimestampedUdpSocket>,
        source_ip_address: SocketAddr,
        sessions: Arc<RwLock<Vec<Session>>>,
        ref_wait: u64,
    ) -> Result<(), CommonError> {
        let socket = self.create_socket()?;
        let rx_token = event_loop.register_event_source(
            socket,
            Box::new(rx_callback(source_ip_address, sessions.clone())),
        )?;
        let timer_spec = Itimerspec {
            it_interval: Duration::from_secs(1),
            it_value: Duration::from_secs(1),
        };
        let _tx_token =
            cleanup_stale_sessions(event_loop, timer_spec, rx_token, sessions, ref_wait)?;
        Ok(())
    }
}

impl Strategy<TwampResult, CommonError> for Reflector {
    fn execute(&mut self) -> std::result::Result<TwampResult, CommonError> {
        // Create the socket
        let source_ip_address = self.configuration.source_ip_address;
        let sessions: Arc<RwLock<Vec<Session>>> = Arc::new(RwLock::new(Vec::new()));
        // Creates the event loop with a default socket
        let mut event_loop = EventLoop::new(1024)?;
        let ref_wait = self.configuration.ref_wait;
        self.create_session(&mut event_loop, source_ip_address, sessions, ref_wait)?;

        // Run the event loop
        event_loop.run()?;

        Ok(TwampResult {
            session_results: Vec::new(),
            error: None,
        })
    }
}

pub fn cleanup_stale_sessions(
    event_loop: &mut EventLoop<TimestampedUdpSocket>,
    timer_spec: Itimerspec,
    rx_token: network_commons::event_loop::Token,
    sessions_clone: Arc<RwLock<Vec<Session>>>,
    ref_wait: u64,
) -> Result<network_commons::event_loop::Token, CommonError> {
    event_loop.register_timer(
        &timer_spec,
        &rx_token,
        Box::new(move |_inner_socket, _| {
            let mut sessions_lock = sessions_clone.write()?;
            sessions_lock.retain(|session| {
                if let Some(session) = session.get_latest_result() {
                    if let Some(packet_results) = session.session.packets {
                        let now = DateTime::utc_now();
                        let last_sent = packet_results.last().and_then(|packet| packet.t2);

                        if let Some(last_sent) = last_sent {
                            let diff = now - last_sent;
                            log::debug!("Diff {:?}, ref_wait: {}, now: {:?}", diff, ref_wait, now);
                            if diff > Duration::from_secs(ref_wait) {
                                return false;
                            }
                        }
                    }
                }
                true
            });
            Ok(0)
        }),
    )
}

pub fn rx_callback(
    rx_socket_address: SocketAddr,
    sessions: Arc<RwLock<Vec<Session>>>,
) -> impl Fn(&mut TimestampedUdpSocket, network_commons::event_loop::Token) -> Result<isize, CommonError>
{
    move |inner_socket: &mut TimestampedUdpSocket, _| {
        let buffer = &mut [0; 1 << 16];
        let (result, socket_address, timestamp) = inner_socket.receive_from(buffer)?;
        log::debug!("Received {} bytes from {}", result, socket_address);
        let (twamp_test_message, _bytes_written): (SenderMessage, usize) =
            SenderMessage::try_from_be_bytes(&buffer[..result.max(0) as usize])?;
        let mut sessions_lock = sessions.write()?;
        let session_option = sessions_lock.iter().find(|session| {
            (session.rx_socket_address == rx_socket_address)
                && (session.tx_socket_address == socket_address)
        });

        if let Some(session) = session_option {
            let reflected_message = ReflectedMessage {
                reflector_sequence_number: session.seq_number.load(Ordering::SeqCst),
                timestamp: NtpTimestamp::from(DateTime::utc_now()),
                error_estimate: ErrorEstimate::new(1, 0, 1, 1),
                mbz1: 0,
                receive_timestamp: NtpTimestamp::from(timestamp),
                sender_sequence_number: twamp_test_message.sequence_number,
                sender_timestamp: twamp_test_message.timestamp,
                sender_error_estimate: twamp_test_message.error_estimate,
                mbz2: 0,
                sender_ttl: 255,
                padding: vec![0_u8; twamp_test_message.padding.len() - MIN_UNAUTH_PADDING],
            };
            log::debug!("Reflected message: \n {:?}", reflected_message);

            inner_socket.send_to(&socket_address, reflected_message.clone())?;
            session.add_to_sent(reflected_message)?;
        } else {
            // Create session
            let session = Session::new(rx_socket_address, socket_address);
            // Create Reflected message
            let reflected_message = ReflectedMessage {
                reflector_sequence_number: session.seq_number.load(Ordering::SeqCst),
                timestamp: NtpTimestamp::from(DateTime::utc_now()),
                error_estimate: ErrorEstimate::new(0, 0, 0, 1),
                mbz1: 0,
                receive_timestamp: NtpTimestamp::from(timestamp),
                sender_sequence_number: twamp_test_message.sequence_number,
                sender_timestamp: twamp_test_message.timestamp,
                sender_error_estimate: twamp_test_message.error_estimate,
                mbz2: 0,
                sender_ttl: 255,
                padding: vec![0; twamp_test_message.padding.len() - MIN_UNAUTH_PADDING],
            };
            log::debug!("Reflected message: \n {:?}", reflected_message);
            // Send message
            inner_socket.send_to(&socket_address, reflected_message.clone())?;
            // Add message results to session
            session.add_to_sent(reflected_message)?;
            // Store session
            sessions_lock.push(session);
        }
        Ok(result)
    }
}

pub struct SessionResult {}

impl TestResult for SessionResult {}
