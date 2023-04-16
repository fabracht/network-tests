use io_uring::{opcode, types};
use slab::Slab;
use std::{collections::HashMap, io, os::fd::AsRawFd};

use crate::{
    epoll_loop::{reset_timer, TimedSources},
    event_loop::{EventLoopTrait, Token},
    socket::Socket,
};

pub struct PolledSource<T: AsRawFd + for<'a> Socket<'a, T>> {
    fd: T,
    pub callback: Box<dyn FnMut(&mut T) -> Result<i32, crate::error::CommonError> + 'static>,
    // pub state: UringState,
}

pub struct UringEventLoop<T: AsRawFd + for<'a> Socket<'a, T>> {
    ring: io_uring::IoUring,
    pub sources: Slab<PolledSource<T>>,
    timed_sources: HashMap<Token, TimedSources<T>>,
}

impl<T: AsRawFd + for<'a> Socket<'a, T>> EventLoopTrait<T> for UringEventLoop<T> {
    fn new(event_capacity: usize) -> Self {
        Self {
            ring: io_uring::IoUring::new(event_capacity.try_into().unwrap()).unwrap(),
            sources: Slab::with_capacity(64),
            timed_sources: HashMap::new(),
        }
    }

    fn generate_token(&mut self) -> Token {
        Token(self.sources.vacant_key())
    }

    fn register_event_source<F>(
        &mut self,
        event_source: T,
        callback: F,
    ) -> Result<Token, crate::error::CommonError>
    where
        F: FnMut(&mut T) -> Result<i32, crate::error::CommonError> + 'static,
    {
        let token = self.generate_token();
        let fd = event_source.as_raw_fd();
        let polled_source = PolledSource {
            fd: event_source,
            callback: Box::new(callback),
        };
        let _ = self.sources.insert(polled_source);
        let poll_e = opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _)
            .multi(true) // Add this line to enable the multi feature
            .build()
            .user_data(token.0 as _);

        let (submitter, mut submit_queue, _) = self.ring.split();

        loop {
            if submit_queue.is_full() {
                submitter.submit()?;
            }
            submit_queue.sync(); // sync with the real current queue
            match unsafe { submit_queue.push(&poll_e) } {
                Ok(_) => break,
                Err(_) => continue,
            };
        }

        Ok(token)
    }

    fn run(&mut self) -> Result<(), crate::error::CommonError> {
        let (submitter, mut _submit_queue, mut completion_queue) = self.ring.split();
        // let mut buffer_pool = Vec::with_capacity(64);
        // let mut buffer_alloc = Slab::with_capacity(64);
        'outer: loop {
            // Submit queued events and wait
            match submitter.submit_and_wait(1) {
                Ok(_) => (),
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(err) => return Err(err.into()),
            }
            // Sync with the real current queue
            completion_queue.sync();

            // Process events from the completion queue
            for completion_event in &mut completion_queue {
                let result = completion_event.result();
                let token_index = completion_event.user_data() as usize;

                // Check for errors in the event result
                if result < 0 {
                    eprintln!(
                        "token {} error {:?}",
                        token_index,
                        io::Error::from_raw_os_error(-result)
                    );
                    continue;
                }

                // let token = Token(token_index);

                if let Some(polled_source) = self.sources.get_mut(token_index) {
                    let fd = &mut polled_source.fd;

                    polled_source.callback.as_mut()(fd)?;
                } else if let Some((timer_source, inner_token, callback)) =
                    self.timed_sources.get_mut(&Token(token_index))
                {
                    if let Some(polled_source) = self.sources.get_mut(inner_token.0) {
                        let fd = &mut polled_source.fd;
                        callback.as_mut()(fd)?;
                    }
                    reset_timer(timer_source)?;
                } else {
                    break 'outer;
                }
            }
        }

        Ok(())
    }

    fn add_duration(
        &mut self,
        time_spec: &crate::event_loop::Itimerspec,
    ) -> Result<Token, crate::error::CommonError> {
        let token = self.generate_token();

        let timespec = types::Timespec::new()
            .nsec(time_spec.it_interval.as_nanos() as u32)
            .sec(time_spec.it_interval.as_nanos() as u64);
        let timeout = opcode::Timeout::new(&timespec as _)
            .flags(types::TimeoutFlags::REALTIME)
            .build();
        let (submitter, mut submit_queue, _) = self.ring.split();

        loop {
            if submit_queue.is_full() {
                submitter.submit()?;
            }
            submit_queue.sync(); // sync with the real current queue
            match unsafe { submit_queue.push(&timeout) } {
                Ok(_) => break,
                Err(_) => continue,
            };
        }

        Ok(token)
    }

    fn add_timer<F>(
        &mut self,
        time_spec: &crate::event_loop::Itimerspec,
        token: &Token,
        callback: F,
    ) -> Result<Token, crate::error::CommonError>
    where
        F: FnMut(&mut T) -> Result<i32, crate::error::CommonError> + 'static,
    {
        let new_token = self.add_duration(time_spec)?;
        if let Some(polled_source) = self.sources.get_mut(token.0) {
            self.timed_sources.insert(
                new_token,
                (polled_source.fd.as_raw_fd(), *token, Box::new(callback)),
            );
        }

        Ok(new_token)
    }
}
