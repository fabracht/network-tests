use std::{
    net::SocketAddr,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        RwLock,
    },
};

use crate::{
    message::{PacketResults, SessionPackets, TimestampsResult},
    time::DateTime,
};

use super::{error::CommonError, host::Host, message::Message};

#[derive(Debug)]
pub struct Session {
    pub socket_address: SocketAddr,
    pub seq_number: AtomicU32,
    pub results: Rc<RwLock<Vec<PacketResults>>>,
}

impl Session {
    pub fn new(host: &Host) -> Self {
        let host = SocketAddr::try_from(host).unwrap();
        Self {
            socket_address: host,
            seq_number: AtomicU32::new(0),
            results: Rc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn from_socket_address(host: &SocketAddr) -> Self {
        Self {
            socket_address: *host,
            seq_number: AtomicU32::new(0),
            results: Rc::new(RwLock::new(Vec::new())),
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

    pub fn get_latest_result(&self) -> TimestampsResult {
        let results = self.results.write().unwrap();
        // if results.len() > 10 {
        //     let b_iter = results.iter().map(|results| {
        //         results
        //             .calculate_owd_backward()
        //             .and_then(|value| value.num_nanoseconds())
        //             .unwrap() as f64
        //     });
        //     let f_iter = results.iter().map(|results| {
        //         results
        //             .calculate_owd_forward()
        //             .and_then(|value| value.num_nanoseconds())
        //             .unwrap() as f64
        //     });
        //     let delays = b_iter.chain(f_iter).collect();

        //     log::error!("Blue offset {} ", offset);
        // }

        let last_result = results.last().unwrap();
        // let ntp = ((last_result.t2.unwrap().get_nanos() - last_result.t1.get_nanos())
        //     + (last_result.t3.unwrap().get_nanos() - last_result.t4.unwrap().get_nanos()))
        //     / 2;
        // let offset_duration = DateTime::from_nanos(ntp.into());
        // log::error!("Symmetric offset = {}", ntp);
        TimestampsResult {
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
        }
    }

    pub fn analyze_packet_loss<'a>(&'_ self) -> Result<(u32, u32, u32), CommonError> {
        let read_lock = self.results.read().map_err(|_| CommonError::Lock)?;
        let mut forward_loss = 0;
        let mut backward_loss = 0;
        let mut total_loss = 0;
        let mut results: Vec<PacketResults> = read_lock.iter().cloned().collect();

        results.sort_unstable_by_key(|p| p.sender_seq);

        for i in 0..results.len() {
            let current = &results[i];
            if current.reflector_seq.is_none() {
                total_loss += 1;
                if i + 1 < results.len() && results[i + 1].reflector_seq == Some(current.sender_seq)
                {
                    forward_loss += 1;
                } else {
                    backward_loss += 1;
                }
            }
        }

        Ok((forward_loss, backward_loss, total_loss))
    }
}
