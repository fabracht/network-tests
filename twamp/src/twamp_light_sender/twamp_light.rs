#[cfg(target_os = "linux")]
use network_commons::epoll_loop::LinuxEventLoop as EventLoop;

use network_commons::{
    error::CommonError,
    event_loop::{EventLoopTrait, Itimerspec, Token},
    socket::{Socket, DEFAULT_BUFFER_SIZE},
    time::NtpTimestamp,
    udp_socket::TimestampedUdpSocket,
    Strategy,
};

use bebytes::BeBytes;

use crate::twamp_common::{
    data_model::ErrorEstimate,
    message::{ReflectedMessage, SenderMessage},
};
use crate::twamp_common::{session::Session, MIN_UNAUTH_PADDING};
use crate::twamp_light_sender::Configuration as TwampLightConfiguration;
use core::time::Duration;
use std::{
    borrow::BorrowMut,
    net::SocketAddr,
    sync::{atomic::Ordering, Arc, RwLock},
};

use super::result::{NetworkStatistics, SessionResult, TwampResult};

pub struct SessionSender {
    /// List of host on which runs a reflecctors to perform the test
    pub targets: Vec<SocketAddr>,
    /// IP address of the interface on which to bind
    pub source_ip_address: SocketAddr,
    /// Interval at which the packets are sent
    pub packet_interval: Duration,
    /// Timeout after which the last message is considered lost
    pub last_message_timeout: Duration,
    /// Padding to add to the packet
    pub padding: usize,
    /// Duration of the test session
    pub duration: Duration,
}

impl SessionSender {
    pub fn new(configuration: &TwampLightConfiguration) -> Self {
        Self {
            targets: configuration.hosts.to_owned(),
            source_ip_address: configuration.source_ip_address.to_owned(),
            duration: Duration::from_secs(configuration.duration),
            packet_interval: Duration::from_millis(configuration.packet_interval),
            padding: configuration.padding,
            last_message_timeout: Duration::from_secs(configuration.last_message_timeout),
        }
    }

    pub fn create_udp_socket(&mut self) -> Result<TimestampedUdpSocket, CommonError> {
        let mut my_socket = TimestampedUdpSocket::bind(&self.source_ip_address)?;

        my_socket.set_fcntl_options()?;
        my_socket.set_socket_options(libc::SOL_IP, libc::IP_RECVERR, Some(1))?;
        my_socket.set_socket_options(libc::IPPROTO_IP, libc::IP_TOS, Some(0))?;

        my_socket.set_timestamping_options()?;

        Ok(my_socket)
    }
}
impl Strategy<TwampResult, CommonError> for SessionSender {
    fn execute(&mut self) -> Result<TwampResult, CommonError> {
        // Create the sessions vector
        let sessions = self
            .targets
            .iter()
            .map(|host| Session::new(self.source_ip_address, *host))
            .collect::<Vec<Session>>();
        let rc_sessions = Arc::new(RwLock::new(sessions));

        // Create the socket
        let my_socket = self.create_udp_socket()?;

        // Creates the event loop with a default socket
        let mut event_loop = EventLoop::new(1024)?;
        event_loop.set_overtime(Itimerspec {
            it_interval: Duration::ZERO,
            it_value: self.last_message_timeout,
        });
        // Register the socket into the event loop
        let rx_token = event_loop
            .register_event_source(my_socket, Box::new(create_rx_callback(rc_sessions.clone())))?;

        // This configures the tx socket timer.
        let timer_spec = Itimerspec {
            it_interval: self.packet_interval,
            it_value: Duration::from_nanos(10),
        };

        // Create the Tx timed event to send twamp messages
        let _tx_token = event_loop.register_timer(
            &timer_spec,
            &rx_token,
            Box::new(create_tx_callback(rc_sessions.clone(), self.padding)),
        )?;

        // // This configures the tx timestamp correction socket timer.
        let tx_correction_timer_spec = Itimerspec {
            it_interval: Duration::from_millis(150),
            it_value: Duration::from_nanos(1),
        };

        let tx_correct_token = event_loop.register_timer(
            &tx_correction_timer_spec,
            &rx_token,
            Box::new(create_tx_correct_callback(rc_sessions.clone())),
        )?;
        event_loop.add_overtime_exception(tx_correct_token);
        // Create the deadline event
        let duration_spec = Itimerspec {
            it_interval: Duration::ZERO,
            it_value: self.duration,
        };

        // Add a deadline event
        let _termination_token = event_loop.add_duration(&duration_spec)?;
        log::info!("Starting test");
        // Run the event loop
        event_loop.run()?;
        log::info!("Test finished");
        log::info!("Calculating results");
        let session_results = calculate_session_results(rc_sessions)?;
        let test_result = TwampResult {
            session_results,
            error: None,
        };

        Ok(test_result)
    }
}

pub fn calculate_session_results(
    rc_sessions: Arc<RwLock<Vec<Session>>>,
) -> Result<Vec<SessionResult>, CommonError> {
    rc_sessions
        .try_read()?
        .borrow_mut()
        .iter()
        .map(|session| -> Result<SessionResult, CommonError> {
            let packets = session.results.try_read()?;
            let total_packets = packets
                .iter()
                .filter(|packet_results| packet_results.t2.is_some() && packet_results.t3.is_some())
                .count();
            let (forward_loss, backward_loss, total_loss) =
                session.analyze_packet_loss().unwrap_or_default();

            let mut rtt_vec = Vec::new();
            let mut f_owd_vec = Vec::new();
            let mut b_owd_vec = Vec::new();
            let mut rpd_vec = Vec::new();
            let mut forward_jitter_vec = Vec::new();
            let mut backward_jitter_vec = Vec::new();

            let mut rtt_sum = 0.0;
            let mut f_owd_sum = 0.0;
            let mut b_owd_sum = 0.0;
            let mut rpd_sum = 0.0;

            let mut prev_forward_owd: Option<f64> = None;
            let mut prev_backward_owd: Option<f64> = None;
            for packet in packets
                .iter()
                .filter(|packet_results| packet_results.t2.is_some() && packet_results.t3.is_some())
            {
                if let Some(rtt) = packet.calculate_rtt() {
                    let rtt = rtt.as_nanos() as f64;
                    rtt_vec.push(rtt);
                    rtt_sum += rtt;
                }

                if let Some(owd) = packet.calculate_owd_forward() {
                    let owd = owd.as_nanos() as f64;
                    f_owd_vec.push(owd);
                    f_owd_sum += owd;

                    // Calculate forward jitter
                    if let Some(prev_fwd) = prev_forward_owd {
                        let fwd_jitter = (owd - prev_fwd).abs();
                        forward_jitter_vec.push(fwd_jitter);
                    }
                    prev_forward_owd = Some(owd);
                }

                if let Some(owd) = packet.calculate_owd_backward() {
                    let owd = owd.as_nanos() as f64;
                    b_owd_vec.push(owd);
                    b_owd_sum += owd;

                    // Calculate backward jitter
                    if let Some(prev_bwd) = prev_backward_owd {
                        let bwd_jitter = (owd - prev_bwd).abs();
                        backward_jitter_vec.push(bwd_jitter);
                    }
                    prev_backward_owd = Some(owd);
                }

                if let Some(rpd) = packet.calculate_rpd() {
                    let rpd = rpd.as_nanos() as f64;
                    rpd_vec.push(rpd);
                    rpd_sum += rpd;
                }
            }

            // Sort the vectors for median and percentile calculations
            rtt_vec.sort_by(|a, b| a.total_cmp(b));
            f_owd_vec.sort_by(|a, b| a.total_cmp(b));
            b_owd_vec.sort_by(|a, b| a.total_cmp(b));
            rpd_vec.sort_by(|a, b| a.total_cmp(b));
            forward_jitter_vec.sort_by(|a, b| a.total_cmp(b));
            backward_jitter_vec.sort_by(|a, b| a.total_cmp(b));

            let gamlr_offset = session.calculate_gamlr_offset(&f_owd_vec, &b_owd_vec);
            let avg_rtt = if total_packets > 0 {
                Some(rtt_sum / (total_packets as f64))
            } else {
                None
            };
            let avg_backward_owd = if total_packets > 0 {
                Some(b_owd_sum / (total_packets as f64))
            } else {
                None
            };
            let avg_forward_owd = if total_packets > 0 {
                Some(f_owd_sum / (total_packets as f64))
            } else {
                None
            };
            let avg_process_time = if total_packets > 0 {
                Some(rpd_sum / (total_packets as f64))
            } else {
                None
            };
            let avg_forward_jitter = if !forward_jitter_vec.is_empty() {
                Some(forward_jitter_vec.iter().sum::<f64>() / forward_jitter_vec.len() as f64)
            } else {
                None
            };
            let avg_backward_jitter = if !backward_jitter_vec.is_empty() {
                Some(backward_jitter_vec.iter().sum::<f64>() / backward_jitter_vec.len() as f64)
            } else {
                None
            };
            let std_dev_forward_jitter = calculate_std_dev(
                &forward_jitter_vec,
                forward_jitter_vec.iter().sum::<f64>() / forward_jitter_vec.len() as f64,
            );
            let std_dev_backward_jitter = calculate_std_dev(
                &backward_jitter_vec,
                backward_jitter_vec.iter().sum::<f64>() / backward_jitter_vec.len() as f64,
            );
            let network_results = NetworkStatistics {
                avg_rtt,
                min_rtt: rtt_vec.iter().min_by(|a, b| a.total_cmp(b)).copied(),
                max_rtt: rtt_vec.iter().max_by(|a, b| a.total_cmp(b)).copied(),
                std_dev_rtt: calculate_std_dev(&rtt_vec, rtt_sum / total_packets as f64),
                median_rtt: median(&rtt_vec),
                low_percentile_rtt: percentile(&rtt_vec, 25.0),
                high_percentile_rtt: percentile(&rtt_vec, 75.0),
                avg_forward_owd,
                min_forward_owd: f_owd_vec.iter().min_by(|a, b| a.total_cmp(b)).copied(),
                max_forward_owd: f_owd_vec.iter().max_by(|a, b| a.total_cmp(b)).copied(),
                std_dev_forward_owd: calculate_std_dev(
                    &f_owd_vec,
                    f_owd_sum / total_packets as f64,
                ),
                median_forward_owd: median(&f_owd_vec),
                low_percentile_forward_owd: percentile(&f_owd_vec, 25.0),
                high_percentile_forward_owd: percentile(&f_owd_vec, 75.0),
                avg_backward_owd,
                min_backward_owd: b_owd_vec.iter().min_by(|a, b| a.total_cmp(b)).copied(),
                max_backward_owd: b_owd_vec.iter().max_by(|a, b| a.total_cmp(b)).copied(),
                std_dev_backward_owd: calculate_std_dev(
                    &b_owd_vec,
                    b_owd_sum / total_packets as f64,
                ),
                median_backward_owd: median(&b_owd_vec),
                low_percentile_backward_owd: percentile(&b_owd_vec, 25.0),
                high_percentile_backward_owd: percentile(&b_owd_vec, 75.0),
                avg_process_time,
                min_process_time: rpd_vec.iter().min_by(|a, b| a.total_cmp(b)).copied(),
                max_process_time: rpd_vec.iter().max_by(|a, b| a.total_cmp(b)).copied(),
                std_dev_process_time: calculate_std_dev(&rpd_vec, rpd_sum / total_packets as f64),
                median_process_time: median(&rpd_vec),
                low_percentile_process_time: percentile(&rpd_vec, 25.0),
                high_percentile_process_time: percentile(&rpd_vec, 75.0),
                avg_forward_jitter,
                avg_backward_jitter,
                std_dev_forward_jitter,
                std_dev_backward_jitter,
                forward_loss,
                backward_loss,
                total_loss,
                total_packets,
                gamlr_offset,
            };

            Ok(SessionResult {
                address: session.tx_socket_address,
                status: Some("Success".to_string()),
                network_statistics: Some(network_results),
            })
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

pub fn create_tx_callback(
    tx_sessions: Arc<RwLock<Vec<Session>>>,
    padding: usize,
) -> impl Fn(&mut TimestampedUdpSocket, Token) -> Result<isize, CommonError> {
    move |inner_socket: &mut TimestampedUdpSocket, _| {
        let mut sent_bytes = vec![];
        let mut timestamps = vec![];
        tx_sessions.try_read()?.iter().for_each(|session| {
            let twamp_test_message = SenderMessage::new(
                session.seq_number.load(Ordering::SeqCst),
                NtpTimestamp::now(),
                ErrorEstimate::new(1, 0, 1, 1),
                vec![0u8; MIN_UNAUTH_PADDING + padding],
            );

            log::trace!("Sending to {}", session.tx_socket_address);
            if let Ok((sent, timestamp)) =
                inner_socket.send_to(&session.tx_socket_address, twamp_test_message)
            {
                sent_bytes.push(sent);
                timestamps.push(timestamp);
                log::trace!("Timestamps {:?}", timestamps);
            } else {
                let error = std::io::Error::last_os_error();
                log::error!(
                    "Error {:#?} sending to {}",
                    error,
                    session.tx_socket_address
                );
            }
        });

        tx_sessions
            .try_read()?
            .iter()
            .zip(timestamps.iter())
            .try_for_each(|(session, timestamp)| {
                let twamp_test_message = SenderMessage {
                    sequence_number: session.seq_number.load(Ordering::SeqCst),
                    timestamp: NtpTimestamp::from(*timestamp),
                    error_estimate: ErrorEstimate::new(1, 0, 1, 1),
                    padding: Vec::new(),
                };
                session.add_to_sent(twamp_test_message)
            })?;
        Ok(0)
    }
}

pub fn create_tx_correct_callback(
    tx_sessions: Arc<RwLock<Vec<Session>>>,
) -> impl Fn(&mut TimestampedUdpSocket, Token) -> Result<isize, CommonError> {
    move |inner_socket: &mut TimestampedUdpSocket, _| {
        let mut tx_timestamps = vec![];
        let addresses_lock = tx_sessions.try_read()?;
        let mut addresses: Vec<SocketAddr> = addresses_lock
            .iter()
            .map(|session| session.tx_socket_address)
            .collect();
        drop(addresses_lock);
        while let Ok(error_messages) = inner_socket.retrieve_tx_timestamps(&mut addresses) {
            log::trace!("Received error messages {}", error_messages.len());
            error_messages.iter().for_each(|tx_timestamp| {
                log::trace!("Received timestamp {:?}", tx_timestamp);
                tx_timestamps.push(tx_timestamp.to_owned());
            });
        }
        let mut write_lock = tx_sessions.try_write()?;
        let length = write_lock.len();

        // mutably iterate through the sessions. Timestamps are ordered by target and then by sequence number
        // so to update the correct ones, we need to iterate through the sessions in the same order
        for i in 0..length {
            let session_timestamps = tx_timestamps
                .iter()
                .skip(i)
                .step_by(length)
                .map(|date_time| date_time.to_owned());

            write_lock.borrow_mut()[i].update_tx_timestamps(session_timestamps)?;
        }
        Ok(0)
    }
}

pub fn create_rx_callback(
    rx_sessions: Arc<RwLock<Vec<Session>>>,
) -> impl Fn(&mut TimestampedUdpSocket, Token) -> Result<isize, CommonError> {
    move |inner_socket, _| {
        let buffer = &mut [0u8; DEFAULT_BUFFER_SIZE];
        while let Ok((result, socket_address, datetime)) = inner_socket.receive_from(buffer) {
            let received_bytes = &buffer[..result as usize];
            let twamp_test_message: &Result<(ReflectedMessage, usize), CommonError> =
                &ReflectedMessage::try_from_be_bytes(received_bytes).map_err(|e| e.into());
            log::trace!("Twamp Response Message {:?}", twamp_test_message);
            if let Ok(twamp_message) = twamp_test_message {
                if let Ok(rw_lock_write_guard) = &rx_sessions.try_write() {
                    log::trace!(
                        "Obtained write lock, looking for session {}",
                        socket_address
                    );
                    let borrowed_sessions = rw_lock_write_guard;
                    let session_option = borrowed_sessions
                        .iter()
                        .find(|session| session.tx_socket_address == socket_address);
                    if let Some(session) = session_option {
                        log::debug!("Received from session {}", session.tx_socket_address);
                        let _ = session.add_to_received(twamp_message.0.to_owned(), datetime);
                        // let latest_result = session.get_latest_result();

                        // if let Ok(json_result) = serde_json::to_string_pretty(&latest_result) {
                        //     log::info!("Latest {}", json_result);
                        // }
                    }
                }
            }
        }
        Ok(0)
    }
}
