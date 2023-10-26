#[cfg(target_os = "linux")]
use network_commons::epoll_loop::LinuxEventLoop as EventLoop;

use network_commons::{
    error::CommonError,
    event_loop::{EventLoopTrait, Itimerspec, Token},
    socket::Socket,
    time::{DateTime, NtpTimestamp},
    udp_socket::TimestampedUdpSocket,
    Strategy,
};

use bebytes::BeBytes;

use crate::twamp_common::message::{ErrorEstimate, ReflectedMessage, SenderMessage};
use crate::twamp_common::{session::Session, MIN_UNAUTH_PADDING};
use crate::twamp_light_sender::Configuration as TwampLightConfiguration;
use core::time::Duration;
use std::{cell::RefCell, net::SocketAddr, os::fd::IntoRawFd, rc::Rc, sync::atomic::Ordering};

use super::result::{NetworkStatistics, SessionResult, TwampResult};

/// The length of the iovec buffer for recvmmsg
const BUFFER_LENGTH: usize = 2;
pub struct TwampLight {
    /// List of host on which runs a reflecctors to perform the test
    hosts: Vec<SocketAddr>,
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

    fn create_socket(&mut self) -> Result<TimestampedUdpSocket, CommonError> {
        let socket = mio::net::UdpSocket::bind(self.source_ip_address.parse().unwrap()).unwrap();
        let mut my_socket = TimestampedUdpSocket::new(socket.into_raw_fd());

        my_socket.set_fcntl_options()?;

        my_socket.set_socket_options(libc::SOL_IP, libc::IP_RECVERR, Some(1))?;

        my_socket.set_timestamping_options()?;

        Ok(my_socket)
    }
}
impl Strategy<TwampResult, CommonError> for TwampLight {
    fn execute(&mut self) -> Result<TwampResult, CommonError> {
        // Create the sessions vector
        let sessions = self
            .hosts
            .iter()
            .filter_map(|host| Session::new(host).ok())
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
            it_value: Duration::from_millis(23),
        };

        // Create the Tx timed event to send twamp messages
        create_tx_callback(
            &mut event_loop,
            timer_spec,
            rx_token,
            rc_sessions.clone(),
            self.padding,
        )?;

        // // This configures the tx timestamp correction socket timer.
        let tx_correction_timer_spec = Itimerspec {
            it_interval: Duration::from_millis(10),
            it_value: Duration::from_nanos(1),
        };

        create_tx_correct_callback(
            &mut event_loop,
            tx_correction_timer_spec,
            rx_token,
            rc_sessions.clone(),
        )?;

        // Create the deadline event
        let duration_spec = Itimerspec {
            it_interval: Duration::ZERO,
            it_value: self.duration,
        };

        // Add a deadline event
        let _termination_token = event_loop.add_duration(&duration_spec)?;
        log::info!("Starting test");
        // Run the event loop
        // std::thread::spawn(move || {
        event_loop.run()?;
        // });
        log::info!("Test finished");
        log::info!("Calculating results");
        let session_results = calculate_session_results(rc_sessions);
        let test_result = TwampResult {
            session_results,
            error: None,
        };

        Ok(test_result)
    }
}
fn calculate_session_results(rc_sessions: Rc<RefCell<Vec<Session>>>) -> Vec<SessionResult> {
    rc_sessions
        .borrow_mut()
        .iter()
        .map(|session| {
            let packets = session.results.read().unwrap();
            let total_packets = packets.len();
            let (forward_loss, backward_loss, total_loss) =
                session.analyze_packet_loss().unwrap_or_default();

            let mut rtt_vec = Vec::new();
            let mut f_owd_vec = Vec::new();
            let mut b_owd_vec = Vec::new();
            let mut rpd_vec = Vec::new();

            let mut rtt_sum = 0.0;
            let mut f_owd_sum = 0.0;
            let mut b_owd_sum = 0.0;
            let mut rpd_sum = 0.0;

            for packet in packets.iter() {
                if let Some(rtt) = packet.calculate_rtt() {
                    let rtt = rtt.as_nanos() as f64;
                    rtt_vec.push(rtt);
                    rtt_sum += rtt;
                }

                if let Some(owd) = packet.calculate_owd_forward() {
                    let owd = owd.as_nanos() as f64;
                    f_owd_vec.push(owd);
                    f_owd_sum += owd;
                }

                if let Some(owd) = packet.calculate_owd_backward() {
                    let owd = owd.as_nanos() as f64;
                    b_owd_vec.push(owd);
                    b_owd_sum += owd;
                }

                if let Some(rpd) = packet.calculate_rpd() {
                    let rpd = rpd.as_nanos() as f64;
                    rpd_vec.push(rpd);
                    rpd_sum += rpd;
                }
            }

            // Sort the vectors for median and percentile calculations
            rtt_vec.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
            f_owd_vec.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
            b_owd_vec.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
            rpd_vec.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

            let gamlr_offset = session.calculate_gamlr_offset(&f_owd_vec, &b_owd_vec);

            let network_results = NetworkStatistics {
                avg_rtt: Some(rtt_sum / total_packets as f64),
                min_rtt: rtt_vec
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                max_rtt: rtt_vec
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                std_dev_rtt: calculate_std_dev(&rtt_vec, rtt_sum / total_packets as f64),
                median_rtt: median(&rtt_vec),
                low_percentile_rtt: percentile(&rtt_vec, 25.0),
                high_percentile_rtt: percentile(&rtt_vec, 75.0),
                avg_forward_owd: Some(f_owd_sum / total_packets as f64),
                min_forward_owd: f_owd_vec
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                max_forward_owd: f_owd_vec
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                std_dev_forward_owd: calculate_std_dev(
                    &f_owd_vec,
                    f_owd_sum / total_packets as f64,
                ),
                median_forward_owd: median(&f_owd_vec),
                low_percentile_forward_owd: percentile(&f_owd_vec, 25.0),
                high_percentile_forward_owd: percentile(&f_owd_vec, 75.0),
                avg_backward_owd: Some(b_owd_sum / total_packets as f64),
                min_backward_owd: b_owd_vec
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                max_backward_owd: b_owd_vec
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                std_dev_backward_owd: calculate_std_dev(
                    &b_owd_vec,
                    b_owd_sum / total_packets as f64,
                ),
                median_backward_owd: median(&b_owd_vec),
                low_percentile_backward_owd: percentile(&b_owd_vec, 25.0),
                high_percentile_backward_owd: percentile(&b_owd_vec, 75.0),
                avg_process_time: Some(rpd_sum / total_packets as f64),
                min_process_time: rpd_vec
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                max_process_time: rpd_vec
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .copied(),
                std_dev_process_time: calculate_std_dev(&rpd_vec, rpd_sum / total_packets as f64),
                median_process_time: median(&rpd_vec),
                low_percentile_process_time: percentile(&rpd_vec, 25.0),
                high_percentile_process_time: percentile(&rpd_vec, 75.0),
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
        .collect()
}

fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        Some((v[mid - 1] + v[mid]) / 2.0)
    } else {
        Some(v[mid])
    }
}

fn percentile(v: &[f64], percentile: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let idx = (percentile / 100.0 * (v.len() - 1) as f64).round() as usize;
    Some(v[idx])
}

fn calculate_std_dev(v: &[f64], mean: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let variance = v.iter().map(|&value| (value - mean).powi(2)).sum::<f64>() / v.len() as f64;
    Some(variance.sqrt())
}

fn create_tx_callback(
    event_loop: &mut EventLoop<TimestampedUdpSocket>,
    timer_spec: Itimerspec,
    rx_token: Token,
    tx_sessions: Rc<RefCell<Vec<Session>>>,
    padding: usize,
) -> Result<usize, CommonError> {
    let tx_token = event_loop.add_timer(&timer_spec, &rx_token, move |inner_socket, _| {
        tx_sessions.borrow().iter().for_each(|session| {
            let send_timestamp = NtpTimestamp::try_from(DateTime::utc_now()).unwrap();
            let twamp_test_message = SenderMessage::new(
                session.seq_number.load(Ordering::SeqCst),
                send_timestamp,
                ErrorEstimate::new(1, 0, 1, 1),
                vec![0u8; MIN_UNAUTH_PADDING + padding],
            );

            log::debug!("Twamp Sender Message {:?}", twamp_test_message);

            if let Ok((_sent, timestamp)) =
                inner_socket.send_to(&session.socket_address, twamp_test_message)
            {
                let twamp_test_message = SenderMessage {
                    sequence_number: session.seq_number.load(Ordering::SeqCst),
                    timestamp: NtpTimestamp::from(timestamp),
                    error_estimate: ErrorEstimate::new(1, 1, 1, 1),
                    padding: Vec::new(),
                };
                session
                    .add_to_sent(twamp_test_message)
                    .expect("Failed to record message to in vector");
            }
        });

        Ok(0)
    })?;
    Ok(tx_token.0)
}

fn create_tx_correct_callback(
    event_loop: &mut EventLoop<TimestampedUdpSocket>,
    timer_spec: Itimerspec,
    rx_token: Token,
    tx_sessions: Rc<RefCell<Vec<Session>>>,
) -> Result<usize, CommonError> {
    let tx_token = event_loop.add_timer(&timer_spec, &rx_token, move |inner_socket, _| {
        let mut tx_timestamps = vec![];

        while let Ok(error_messages) = inner_socket.receive_errors() {
            // log::warn!("Received error messages {}", error_messages.len());
            error_messages
                .iter()
                .for_each(|(_res, _address, tx_timestamp)| {
                    tx_timestamps.push(tx_timestamp.to_owned());
                });
        }
        let length = tx_sessions.borrow().len();
        // mutably iterate through the sessions. Timestamps are ordered by target and then by sequence number
        // so to update the correct ones, we need to iterate through the sessions in the same order
        for i in 0..length {
            let session_timestamps = tx_timestamps
                .iter()
                .skip(i)
                .step_by(length)
                .map(|date_time| date_time.to_owned());

            tx_sessions.borrow_mut()[i].update_tx_timestamps(session_timestamps)?;
        }
        Ok(0)
    })?;
    Ok(tx_token.0)
}

fn create_rx_callback(
    event_loop: &mut EventLoop<TimestampedUdpSocket>,
    my_socket: TimestampedUdpSocket,
    rx_sessions: Rc<RefCell<Vec<Session>>>,
) -> Result<Token, CommonError> {
    let rx_token = event_loop.register_event_source(my_socket, move |inner_socket, _| {
        let buffers = &mut [[0u8; 1024]; BUFFER_LENGTH];
        while let Ok(response_vec) = inner_socket.receive_from_multiple(buffers, BUFFER_LENGTH) {
            response_vec.iter().enumerate().for_each(
                |(i, (result, socket_address, timespec_ref))| {
                    let received_bytes = &buffers[i][..*result];
                    let twamp_test_message: &Result<(ReflectedMessage, usize), CommonError> =
                        &ReflectedMessage::try_from_be_bytes(received_bytes).map_err(|e| e.into());
                    log::debug!("Twamp Response Message {:?}", twamp_test_message);
                    if let Ok(twamp_message) = twamp_test_message {
                        let borrowed_sessions = rx_sessions.borrow();
                        let session_option = borrowed_sessions
                            .iter()
                            .find(|session| session.socket_address == *socket_address);
                        if let Some(session) = session_option {
                            session
                                .add_to_received(
                                    twamp_message.0.to_owned(),
                                    DateTime::from_timespec(*timespec_ref),
                                )
                                .unwrap();
                            let latest_result = session.get_latest_result();

                            let json_result = serde_json::to_string_pretty(&latest_result).unwrap();
                            log::debug!("Latest {}", json_result);
                        }
                    }
                },
            );
        }
        Ok(0)
    })?;
    Ok(rx_token)
}
