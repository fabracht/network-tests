use std::{
    net::SocketAddr,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        RwLock,
    },
};

use common::{
    error::CommonError,
    host::Host,
    message::{Message, PacketResults, SessionPackets, TimestampsResult},
    stats::{offset_estimator::estimate, statistics::OrderStatisticsTree},
    time::DateTime,
};

#[derive(Debug)]
pub struct Session {
    pub socket_address: SocketAddr,
    pub seq_number: AtomicU32,
    pub results: Rc<RwLock<Vec<PacketResults>>>,
    pub last_updated: usize,
}

impl Session {
    pub fn new(host: &Host) -> Self {
        let host = SocketAddr::try_from(host).unwrap();
        Self {
            socket_address: host,
            seq_number: AtomicU32::new(0),
            results: Rc::new(RwLock::new(Vec::new())),
            last_updated: 0,
        }
    }

    pub fn from_socket_address(host: &SocketAddr) -> Self {
        Self {
            socket_address: *host,
            seq_number: AtomicU32::new(0),
            results: Rc::new(RwLock::new(Vec::new())),
            last_updated: 0,
        }
    }

    pub fn add_to_received(&self, message: impl Message, t4: DateTime) -> Result<(), CommonError> {
        self.results.write()?.iter_mut().for_each(|result| {
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

    pub fn add_to_sent(&self, message: Box<dyn Message>) {
        let packet_result = message.packet_results();

        self.results.write().unwrap().push(packet_result);
        self.seq_number.fetch_add(1, Ordering::Relaxed);
    }

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

    pub fn analyze_packet_loss<'a>(&'_ self) -> Result<(u32, u32, u32), CommonError> {
        let read_lock = self.results.read().map_err(|_| CommonError::Lock)?;
        let mut forward_loss: i32 = 0;
        let backward_loss;
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
                        let delta = ((current.sender_seq as i32 - last_sender_seq as i32)
                            - (current_reflector_seq as i32 - last_reflector_seq as i32))
                            as i32;

                        if delta >= 0 {
                            forward_loss += delta;
                        }
                    }
                }

                last_successful_sender_seq = Some(current.sender_seq);
                last_successful_reflector_seq = current.reflector_seq;
            }
        }

        backward_loss = total_loss - forward_loss;

        Ok((forward_loss as u32, backward_loss as u32, total_loss as u32))
    }

    pub fn calculate_gamlr_offset(&self) -> Option<f64> {
        if let Ok(results) = self.results.read() {
            if results.is_empty() || results.len() < 5 {
                return None;
            }
            let mut f_owd_tree = OrderStatisticsTree::new();
            let mut b_owd_tree = OrderStatisticsTree::new();
            let packets = self.results.read().unwrap().clone();

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
            let forward_owd: Vec<f64> = f_owd_tree
                .iter(common::stats::tree_iterator::TraversalOrder::Inorder)
                .map(|node| node.value())
                .collect();
            let backward_owd: Vec<f64> = b_owd_tree
                .iter(common::stats::tree_iterator::TraversalOrder::Inorder)
                .map(|node| node.value())
                .collect();

            let mut f_offset = 0.0;
            let mut b_offset = 0.0;

            for slice in forward_owd.chunks(5) {
                f_offset += estimate(slice.to_owned());
            }
            for slice in backward_owd.chunks(5) {
                b_offset += estimate(slice.to_owned());
            }

            f_offset /= forward_owd.len() as f64;
            b_offset /= backward_owd.len() as f64;

            return Some((f_offset - b_offset) / 2.0);
        }
        None
    }
}
