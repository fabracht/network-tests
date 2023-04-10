#[cfg(target_os = "linux")]
use common::epoll_loop::LinuxEventLoop as EventLoop;
use common::{
    error::CommonError,
    event_loop::{EventLoopTrait, Itimerspec, Token},
    host::Host,
    session::Session,
    socket::{set_timestamping_options, CustomUdpSocket, Socket},
    statistics::OrderStatisticsTree,
    time::{DateTime, NtpTimestamp},
    Strategy,
};

#[cfg(target_os = "macos")]
use common::kevent_loop::MacOSEventLoop as EventLoop;

use core::time::Duration;
use std::{os::fd::IntoRawFd, rc::Rc, sync::atomic::Ordering};

use crate::common::{ErrorEstimate, ReflectedMessage, SenderMessage, CONST_PADDING};

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
}

impl TwampLight {
    pub fn new(
        source_ip_address: &str,
        duration: Duration,
        hosts: &[Host],
        packet_interval: Duration,
    ) -> Self {
        Self {
            hosts: hosts.to_owned(),
            source_ip_address: source_ip_address.to_owned(),
            duration,
            packet_interval,
        }
    }

    fn create_socket(&mut self) -> Result<CustomUdpSocket, crate::CommonError> {
        let socket = mio::net::UdpSocket::bind(self.source_ip_address.parse().unwrap()).unwrap();
        let mut my_socket = CustomUdpSocket::new(socket.into_raw_fd());

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
        let rc_sessions = Rc::new(sessions);

        // Create the socket
        let my_socket = self.create_socket()?;

        // Create the poll
        // let poll = Poll::new()?;
        // Creates the event loop with a default socket
        let mut event_loop = EventLoop::new(1024);

        // Register the socket into the event loop
        let rx_token = create_rx_callback(&mut event_loop, my_socket, rc_sessions.clone())?;

        // This configures the tx socket timer.
        let timer_spec = Itimerspec {
            it_interval: self.packet_interval,
            it_value: Duration::from_micros(1),
        };

        // Create the Tx timed event to send twamp messages
        create_tx_callback(&mut event_loop, timer_spec, rx_token, rc_sessions.clone())?;

        let duration_spec = Itimerspec {
            it_interval: Duration::from_micros(1),
            it_value: self.duration,
        };

        // Add a deadline event
        let _termination_token = event_loop.add_duration(&duration_spec)?;

        // Run the event loop
        event_loop.run()?;

        let session_results = rc_sessions
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

                rtt_tree.insert_all(packets.iter().flat_map(|packet| {
                    packet
                        .calculate_rtt()
                        .and_then(|rtt| Some(rtt.as_nanos() as u32))
                }));
                f_owd_tree.insert_all(packets.iter().flat_map(|packet| {
                    packet
                        .calculate_owd_forward()
                        .and_then(|owd| Some(owd.as_nanos() as u32))
                }));
                b_owd_tree.insert_all(packets.iter().flat_map(|packet| {
                    packet
                        .calculate_owd_backward()
                        .and_then(|owd| Some(owd.as_nanos() as u32))
                }));
                rpd_tree.insert_all(packets.iter().flat_map(|packet| {
                    packet
                        .calculate_rpd()
                        .and_then(|rpd| Some(rpd.as_nanos() as u32))
                }));

                let network_results = NetworkStatistics {
                    avg_rtt: rtt_tree.mean(),
                    min_rtt: rtt_tree.min().unwrap_or_default(),
                    max_rtt: rtt_tree.max().unwrap_or_default(),
                    std_dev_rtt: rtt_tree.std_dev(),
                    median_rtt: rtt_tree.median().unwrap_or_default(),
                    low_percentile_rtt: rtt_tree.percentile(0.25).unwrap_or_default(),
                    high_percentile_rtt: rtt_tree.percentile(0.75).unwrap_or_default(),
                    avg_forward_owd: f_owd_tree.mean(),
                    min_forward_owd: f_owd_tree.min().unwrap_or_default(),
                    max_forward_owd: f_owd_tree.max().unwrap_or_default(),
                    std_dev_forward_owd: f_owd_tree.std_dev(),
                    median_forward_owd: f_owd_tree.median().unwrap_or_default(),
                    low_percentile_forward_owd: f_owd_tree.percentile(0.25).unwrap_or_default(),
                    high_percentile_forward_owd: f_owd_tree.percentile(0.75).unwrap_or_default(),
                    avg_backward_owd: b_owd_tree.mean(),
                    min_backward_owd: b_owd_tree.min().unwrap_or_default(),
                    max_backward_owd: b_owd_tree.max().unwrap_or_default(),
                    std_dev_backward_owd: b_owd_tree.std_dev(),
                    median_backward_owd: b_owd_tree.median().unwrap_or_default(),
                    low_percentile_backward_owd: b_owd_tree.percentile(0.25).unwrap_or_default(),
                    high_percentile_backward_owd: b_owd_tree.percentile(0.75).unwrap_or_default(),
                    avg_process_time: rpd_tree.mean(),
                    min_process_time: rpd_tree.min().unwrap_or_default(),
                    max_process_time: rpd_tree.max().unwrap_or_default(),
                    std_dev_process_time: rpd_tree.std_dev(),
                    median_process_time: rpd_tree.median().unwrap_or_default(),
                    low_percentile_process_time: rpd_tree.percentile(0.25).unwrap_or_default(),
                    high_percentile_process_time: rpd_tree.percentile(0.75).unwrap_or_default(),
                    forward_loss,
                    backward_loss,
                    total_loss,
                    total_packets,
                };

                SessionResult {
                    address: session.socket_address,
                    status: Some("Success".to_string()),
                    network_statistics: Some(network_results),
                }
            })
            .collect::<Vec<SessionResult>>();
        let test_result = TwampResult {
            session_results,
            error: None,
        };

        Ok(test_result)
    }
}

fn create_tx_callback(
    event_loop: &mut EventLoop<CustomUdpSocket>,
    timer_spec: Itimerspec,
    rx_token: Token,
    tx_sessions: Rc<Vec<Session>>,
) -> Result<usize, CommonError> {
    let _tx_token = event_loop.add_timer(&timer_spec, &rx_token, move |inner_socket| {
        let mut received_bytes = vec![];
        let mut timestamps = vec![];

        let timestamp = NtpTimestamp::try_from(DateTime::utc_now())?;
        tx_sessions.iter().for_each(|session| {
            let twamp_test_message = SenderMessage {
                timestamp: NtpTimestamp::try_from(DateTime::utc_now()).unwrap_or(timestamp),
                sequence_number: session.seq_number.load(Ordering::SeqCst),
                error_estimate: ErrorEstimate::new(1, 0, 1, 1).unwrap(),
                padding: vec![0u8; CONST_PADDING],
            };
            log::debug!("Twamp Response Message {:?}", twamp_test_message);
            let (sent, timestamp) = inner_socket
                .send_to(&session.socket_address, twamp_test_message.clone())
                .unwrap();

            received_bytes.push(sent);
            timestamps.push(timestamp);
        });
        tx_sessions
            .iter()
            .zip(timestamps.iter())
            .for_each(|(session, timestamp)| {
                let twamp_test_message = SenderMessage {
                    sequence_number: session.seq_number.load(Ordering::SeqCst),
                    timestamp: NtpTimestamp::from(timestamp.clone()),
                    error_estimate: ErrorEstimate::new(1, 1, 1, 1).unwrap(),
                    padding: vec![0u8; CONST_PADDING],
                };
                session.add_to_sent(Box::new(twamp_test_message))
            });
        Ok(0)
    })?;
    Ok(5)
}

fn create_rx_callback(
    event_loop: &mut EventLoop<CustomUdpSocket>,
    my_socket: CustomUdpSocket,
    rx_sessions: Rc<Vec<Session>>,
) -> Result<Token, CommonError> {
    let rx_token = event_loop.register_event_source(my_socket, move |inner_socket| {
        let buffer = &mut [0; 1024];
        let (result, socket_address, timestamp) = inner_socket.receive_from(buffer)?;
        let twamp_test_message: &Result<ReflectedMessage, CommonError> =
            &buffer[..result].try_into();
        log::info!("Twamp Response Message {:?}", twamp_test_message);
        // print_bytes(&buffer[..result]);
        if let Ok(twamp_message) = twamp_test_message {
            let session_option = rx_sessions
                .iter()
                .find(|session| session.socket_address == socket_address);
            if let Some(session) = session_option {
                session.add_to_received(twamp_message.to_owned(), timestamp)?;
                let latest_result = session.get_latest_result();

                let json_result = serde_json::to_string_pretty(&latest_result).unwrap();
                log::warn!("Latest {}", json_result);
            }
        }
        Ok(result as i32)
    })?;
    Ok(rx_token)
}
