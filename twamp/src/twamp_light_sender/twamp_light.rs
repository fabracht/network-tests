#[cfg(target_os = "linux")]
use common::epoll_loop::LinuxEventLoop as EventLoop;

use common::{
    error::CommonError,
    event_loop::{EventLoopTrait, Itimerspec, Token},
    host::Host,
    socket::Socket,
    stats::statistics::OrderStatisticsTree,
    time::{DateTime, NtpTimestamp},
    udp_socket::{set_timestamping_options, TimestampedUdpSocket},
    Strategy,
};

#[cfg(target_os = "macos")]
use common::kevent_loop::MacOSEventLoop as EventLoop;
use message_macro::BeBytes;

use crate::common::message::{ErrorEstimate, ReflectedMessage, SenderMessage};
use crate::common::{session::Session, MIN_UNAUTH_PADDING};
use crate::twamp_light_sender::Configuration as TwampLightConfiguration;
use core::time::Duration;
use std::{cell::RefCell, os::fd::IntoRawFd, rc::Rc, sync::atomic::Ordering};

use super::result::{NetworkStatistics, SessionResult, TwampResult};

pub struct TwampLight {
    /// List of host on which runs a reflecctors to perform the test
    hosts: Vec<Host>,
    /// IP address of the interface on which to bind
    source_ip_address: String,
    /// Duration of the test
    duration: Duration,
    /// Interval at which the packets are sent
    packet_interval: Duration,
    /// Timeout after which the last message is considered lost
    last_message_timeout: Duration,
    /// Padding to add to the packet
    padding: usize,
}

impl TwampLight {
    pub fn new(configuration: &TwampLightConfiguration) -> Self {
        Self {
            hosts: configuration.hosts.to_owned(),
            source_ip_address: configuration.source_ip_address.to_owned(),
            duration: Duration::from_secs(configuration.duration),
            packet_interval: Duration::from_millis(configuration.packet_interval),
            padding: configuration.padding,
            last_message_timeout: Duration::from_secs(configuration.last_message_timeout),
        }
    }

    fn create_socket(&mut self) -> Result<TimestampedUdpSocket, crate::CommonError> {
        let socket = mio::net::UdpSocket::bind(self.source_ip_address.parse().unwrap()).unwrap();
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
impl Strategy<TwampResult, crate::CommonError> for TwampLight {
    fn execute(&mut self) -> Result<TwampResult, crate::CommonError> {
        // Create the sessions vector
        let sessions = self
            .hosts
            .iter()
            .map(Session::new)
            .collect::<Vec<Session>>();
        let rc_sessions = Rc::new(RefCell::new(sessions));

        // Create the socket
        let my_socket = self.create_socket()?;

        // Creates the event loop with a default socket
        let mut event_loop = EventLoop::new(1024)?;
        event_loop.set_overtime(Itimerspec {
            it_interval: self.last_message_timeout,
            it_value: self.last_message_timeout,
        });
        // Register the socket into the event loop
        let rx_token = create_rx_callback(&mut event_loop, my_socket, rc_sessions.clone())?;

        // This configures the tx socket timer.
        let timer_spec = Itimerspec {
            it_interval: self.packet_interval,
            it_value: Duration::from_micros(1),
        };

        // Create the Tx timed event to send twamp messages
        create_tx_callback(
            &mut event_loop,
            timer_spec,
            rx_token,
            rc_sessions.clone(),
            self.padding,
        )?;

        let duration_spec = Itimerspec {
            it_interval: Duration::ZERO,
            it_value: self.duration,
        };

        // Add a deadline event
        let _termination_token = event_loop.add_duration(&duration_spec)?;

        // Run the event loop
        event_loop.run()?;

        let session_results = calculate_session_results(rc_sessions);
        let test_result = TwampResult {
            session_results,
            error: None,
        };

        Ok(test_result)
    }
}

fn calculate_session_results(rc_sessions: Rc<RefCell<Vec<Session>>>) -> Vec<SessionResult> {
    let session_results = rc_sessions
        .borrow_mut()
        .iter()
        .map(|session| {
            let packets = session.results.read().unwrap().clone();
            let total_packets = packets.len();
            let (forward_loss, backward_loss, total_loss) =
                session.analyze_packet_loss().unwrap_or_default();
            let mut rtt_tree = OrderStatisticsTree::new();
            let mut f_owd_tree = OrderStatisticsTree::new();
            let mut b_owd_tree = OrderStatisticsTree::new();
            let mut rpd_tree = OrderStatisticsTree::new();

            rtt_tree.insert_all(
                packets
                    .iter()
                    .flat_map(|packet| packet.calculate_rtt().map(|rtt| rtt.as_nanos() as u32)),
            );
            f_owd_tree.insert_all(packets.iter().flat_map(|packet| {
                packet
                    .calculate_owd_forward()
                    .map(|owd| owd.as_nanos() as u32)
            }));
            b_owd_tree.insert_all(packets.iter().flat_map(|packet| {
                packet
                    .calculate_owd_backward()
                    .map(|owd| owd.as_nanos() as u32)
            }));
            rpd_tree.insert_all(
                packets
                    .iter()
                    .flat_map(|packet| packet.calculate_rpd().map(|rpd| rpd.as_nanos() as u32)),
            );
            let gamlr_offset = session.calculate_gamlr_offset();
            let network_results = NetworkStatistics {
                avg_rtt: rtt_tree.mean(),
                min_rtt: rtt_tree.min().unwrap_or_default(),
                max_rtt: rtt_tree.max().unwrap_or_default(),
                std_dev_rtt: rtt_tree.std_dev(),
                median_rtt: rtt_tree.median().unwrap_or_default(),
                low_percentile_rtt: rtt_tree.percentile(25.0).unwrap_or_default(),
                high_percentile_rtt: rtt_tree.percentile(75.0).unwrap_or_default(),
                avg_forward_owd: f_owd_tree.mean(),
                min_forward_owd: f_owd_tree.min().unwrap_or_default(),
                max_forward_owd: f_owd_tree.max().unwrap_or_default(),
                std_dev_forward_owd: f_owd_tree.std_dev(),
                median_forward_owd: f_owd_tree.median().unwrap_or_default(),
                low_percentile_forward_owd: f_owd_tree.percentile(25.0).unwrap_or_default(),
                high_percentile_forward_owd: f_owd_tree.percentile(75.0).unwrap_or_default(),
                avg_backward_owd: b_owd_tree.mean(),
                min_backward_owd: b_owd_tree.min().unwrap_or_default(),
                max_backward_owd: b_owd_tree.max().unwrap_or_default(),
                std_dev_backward_owd: b_owd_tree.std_dev(),
                median_backward_owd: b_owd_tree.median().unwrap_or_default(),
                low_percentile_backward_owd: b_owd_tree.percentile(25.0).unwrap_or_default(),
                high_percentile_backward_owd: b_owd_tree.percentile(75.0).unwrap_or_default(),
                avg_process_time: rpd_tree.mean(),
                min_process_time: rpd_tree.min().unwrap_or_default(),
                max_process_time: rpd_tree.max().unwrap_or_default(),
                std_dev_process_time: rpd_tree.std_dev(),
                median_process_time: rpd_tree.median().unwrap_or_default(),
                low_percentile_process_time: rpd_tree.percentile(25.0).unwrap_or_default(),
                high_percentile_process_time: rpd_tree.percentile(75.0).unwrap_or_default(),
                forward_loss,
                backward_loss,
                total_loss,
                total_packets,
                gamlr_offset,
            };

            SessionResult {
                address: session.socket_address,
                status: Some("Success".to_string()),
                network_statistics: Some(network_results),
            }
        })
        .collect::<Vec<SessionResult>>();
    session_results
}

fn create_tx_callback(
    event_loop: &mut EventLoop<TimestampedUdpSocket>,
    timer_spec: Itimerspec,
    rx_token: Token,
    tx_sessions: Rc<RefCell<Vec<Session>>>,
    padding: usize,
) -> Result<usize, CommonError> {
    let _tx_token = event_loop.add_timer(&timer_spec, &rx_token, move |inner_socket, _| {
        let mut received_bytes = vec![];
        let mut timestamps = vec![];

        let timestamp = NtpTimestamp::try_from(DateTime::utc_now())?;
        tx_sessions.borrow().iter().for_each(|session| {
            let twamp_test_message = SenderMessage::new(
                session.seq_number.load(Ordering::SeqCst),
                timestamp,
                ErrorEstimate::new(1, 0, 1, 1).unwrap(),
                vec![0u8; MIN_UNAUTH_PADDING + padding],
            )
            .map_err(|e| CommonError::from(e.to_string()));

            log::debug!("Twamp Sender Message {:?}", twamp_test_message);
            let (sent, timestamp) = inner_socket
                .send_to(&session.socket_address, twamp_test_message.unwrap())
                .unwrap();

            received_bytes.push(sent);
            timestamps.push(timestamp);
        });
        tx_sessions
            .borrow()
            .iter()
            .zip(timestamps.iter())
            .for_each(|(session, timestamp)| {
                let twamp_test_message = SenderMessage {
                    sequence_number: session.seq_number.load(Ordering::SeqCst),
                    timestamp: NtpTimestamp::from(*timestamp),
                    error_estimate: ErrorEstimate::new(1, 1, 1, 1).unwrap(),
                    padding: vec![0u8; 0],
                };
                session.add_to_sent(Box::new(twamp_test_message))
            });
        Ok(0)
    })?;
    Ok(5)
}

fn create_rx_callback(
    event_loop: &mut EventLoop<TimestampedUdpSocket>,
    my_socket: TimestampedUdpSocket,
    rx_sessions: Rc<RefCell<Vec<Session>>>,
) -> Result<Token, CommonError> {
    let rx_token = event_loop.register_event_source(my_socket, move |inner_socket, _| {
        let buffer = &mut [0; 1024];
        let (result, socket_address, timestamp) = inner_socket.receive_from(buffer)?;
        log::info!("Received {} bytes from {}", result, socket_address);
        let twamp_test_message: &Result<(ReflectedMessage, usize), CommonError> =
            &ReflectedMessage::try_from_be_bytes(&buffer[..result]).map_err(|e| e.into());
        log::info!("Twamp Response Message {:?}", twamp_test_message);
        if let Ok(twamp_message) = twamp_test_message {
            let borrowed_sessions = rx_sessions.borrow();
            let session_option = borrowed_sessions
                .iter()
                .find(|session| session.socket_address == socket_address);
            if let Some(session) = session_option {
                session.add_to_received(twamp_message.0.to_owned(), timestamp)?;
                let latest_result = session.get_latest_result();

                let json_result = serde_json::to_string_pretty(&latest_result).unwrap();
                log::debug!("Latest {}", json_result);
            }
        }
        Ok(result as i32)
    })?;
    Ok(rx_token)
}
