use network_commons::{
    error::CommonError,
    message::{Message, PacketResults, SessionPackets, TimestampsResult},
    stats::offset_estimator::estimate,
    time::DateTime,
};
use std::{
    net::SocketAddr,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        RwLock,
    },
};

/// A `Session` represents a communication session with a `Host`.
/// It maintains a sequence number and a collection of `PacketResults`.
/// A session also provides several methods for adding new packets to the session,
/// getting the latest result, and analyzing packet loss.
#[derive(Debug)]
pub struct Session {
    pub socket_address: SocketAddr,
    pub seq_number: AtomicU32,
    pub results: Rc<RwLock<Vec<PacketResults>>>,
    pub last_updated: usize,
}

impl Session {
    /// Creates a new `Session` from a `Host`.
    pub fn new(host: &SocketAddr) -> Result<Self, CommonError> {
        Ok(Self {
            socket_address: host.clone(),
            seq_number: AtomicU32::new(0),
            results: Rc::new(RwLock::new(Vec::new())),
            last_updated: 0,
        })
    }

    /// Creates a new `Session` from a `SocketAddr`.
    pub fn from_socket_address(host: &SocketAddr) -> Self {
        Self {
            socket_address: *host,
            seq_number: AtomicU32::new(0),
            results: Rc::new(RwLock::new(Vec::new())),
            last_updated: 0,
        }
    }

    /// Adds a received packet to the session's results.
    /// The method finds the matching sent packet by sequence number and updates its fields.
    pub fn add_to_received(&self, message: impl Message, t4: DateTime) -> Result<(), CommonError> {
        let mut write_lock = self.results.write()?;
        write_lock.iter_mut().for_each(|result| {
            let packet_results = message.packet_results();

            if result.sender_seq == packet_results.sender_seq {
                result.reflector_seq = packet_results.reflector_seq;
                result.t2 = packet_results.t2;
                result.t3 = packet_results.t3;
                result.t4 = Some(t4);
            }
        });
        Ok(())
    }

    /// Adds a sent packet to the session's results and increments the sequence number.
    pub fn add_to_sent(&self, message: Box<dyn Message>) -> Result<(), CommonError> {
        let packet_result = message.packet_results();

        self.results
            .write()
            .map(|mut results| results.push(packet_result))?;
        self.seq_number.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Gets the most recent result of this session.
    pub fn get_latest_result(&self) -> Option<TimestampsResult> {
        let results = self.results.write().ok()?;
        let last_result = results.last()?;
        Some(TimestampsResult {
            session: SessionPackets {
                address: self.socket_address,
                packets: Some(vec![PacketResults {
                    sender_seq: last_result.sender_seq,
                    reflector_seq: last_result.reflector_seq,
                    t1: last_result.t1,
                    t2: last_result.t2,
                    t3: last_result.t3,
                    t4: last_result.t4,
                }]),
            },
            error: None,
        })
    }

    /// Updates the transmit timestamps for the packet results based on the provided iterator.
    pub fn update_tx_timestamps(
        &mut self,
        mut timestamps: impl Iterator<Item = DateTime>,
    ) -> Result<(), CommonError> {
        let mut results = self.results.write()?;
        for result in results.iter_mut().skip(self.last_updated) {
            if let Some(timestamp) = timestamps.next() {
                let delta = timestamp - result.t1;
                log::trace!("Delta: {:?}", delta);

                result.t1 = timestamp;
                self.last_updated += 1;
            }
        }

        Ok(())
    }

    /// Analyzes the packet loss in this session.
    /// Returns a tuple containing the counts of forward, backward, and total lost packets.
    pub fn analyze_packet_loss(&'_ self) -> Result<(u32, u32, u32), CommonError> {
        let read_lock = self.results.read().map_err(|_| CommonError::Lock)?;
        let mut forward_loss: i32 = 0;

        let mut total_loss = 0;
        let mut results: Vec<PacketResults> = read_lock.iter().cloned().collect();

        results.sort_unstable_by_key(|p| p.sender_seq);

        let mut last_successful_sender_seq: Option<u32> = None;
        let mut last_successful_reflector_seq: Option<u32> = None;

        // Check if the first packet is lost and increment the total_loss counter accordingly
        if results
            .get(0)
            .map(|p| p.reflector_seq.is_none())
            .unwrap_or(false)
        {
            total_loss += 1;
        }

        for current in results.iter().skip(1) {
            if current.reflector_seq.is_none() {
                total_loss += 1;
            } else {
                if let Some(last_sender_seq) = last_successful_sender_seq {
                    if let Some(last_reflector_seq) = last_successful_reflector_seq {
                        let current_reflector_seq = current.reflector_seq.unwrap_or(0);
                        let delta = (current.sender_seq as i32 - last_sender_seq as i32)
                            - (current_reflector_seq as i32 - last_reflector_seq as i32);

                        if delta >= 0 {
                            forward_loss += delta;
                        }
                    }
                }

                last_successful_sender_seq = Some(current.sender_seq);
                last_successful_reflector_seq = current.reflector_seq;
            }
        }

        let backward_loss = total_loss - forward_loss;

        Ok((forward_loss as u32, backward_loss as u32, total_loss as u32))
    }

    /// Calculates the GAMLR offset for this session.
    /// Uses the provided OrderStatisticsTrees for forward and backward One-Way Delay.
    pub fn calculate_gamlr_offset(&self, forward_owd: &[f64], backward_owd: &[f64]) -> Option<f64> {
        // let results = self.results.read().ok()?;
        if forward_owd.len() < 5 || backward_owd.len() < 5 {
            return None;
        }
        // if results.is_empty() || results.len() < 5 {
        //     return None;
        // }

        // let forward_owd: Vec<f64> = f_owd_tree.iter(Inorder).map(|node| node.value()).collect();
        // let backward_owd: Vec<f64> = b_owd_tree.iter(Inorder).map(|node| node.value()).collect();

        // Ensure that we have complete chunks for the estimate
        let f_chunks: Vec<_> = forward_owd
            .chunks(5)
            .filter(|chunk| chunk.len() == 5)
            .collect();
        let f_len = f_chunks.len();
        let b_chunks: Vec<_> = backward_owd
            .chunks(5)
            .filter(|chunk| chunk.len() == 5)
            .collect();
        let b_len = b_chunks.len();

        let mut f_offset = 0.0;
        let mut b_offset = 0.0;

        for slice in f_chunks {
            f_offset += estimate(slice.to_owned());
        }
        for slice in b_chunks {
            b_offset += estimate(slice.to_owned());
        }

        f_offset /= f_len as f64;
        b_offset /= b_len as f64;

        Some((f_offset - b_offset) / 2.0)
    }
}
