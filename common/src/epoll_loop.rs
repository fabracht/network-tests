use mio::{unix::SourceFd, Events, Interest};
use std::{
    collections::HashMap,
    os::fd::{AsRawFd, RawFd},
};

use crate::{
    error::CommonError,
    event_loop::{itimerspec_to_libc, EventLoopTrait, Itimerspec, Token},
    libc_call,
    socket::Socket,
};

pub type Sources<T> = (T, Box<dyn FnMut(&mut T) -> Result<i32, CommonError>>);
pub type TimedSources<T> = (
    RawFd,
    Token,
    Box<dyn FnMut(&mut T) -> Result<i32, CommonError>>,
);

pub struct LinuxEventLoop<T: AsRawFd + for<'a> Socket<'a, T>> {
    poll: mio::Poll,
    events: Events,
    pub sources: HashMap<Token, Sources<T>>,
    timed_sources: HashMap<Token, TimedSources<T>>,
    next_token: usize,
}
impl<T: AsRawFd + for<'a> Socket<'a, T>> EventLoopTrait<T> for LinuxEventLoop<T> {
    fn new(event_capacity: usize) -> Self {
        // Create the poll
        let poll = mio::Poll::new().unwrap();

        let events = Events::with_capacity(event_capacity);

        Self {
            poll,
            events,
            sources: HashMap::new(),
            timed_sources: HashMap::new(),
            next_token: 0,
        }
    }

    fn generate_token(&mut self) -> Token {
        let token = Token(self.next_token);
        self.next_token += 1;
        token
    }

    fn register_event_source<F>(
        &mut self,
        event_source: T,
        callback: F,
    ) -> Result<Token, CommonError>
    where
        F: FnMut(&mut T) -> Result<i32, CommonError> + 'static,
    {
        let binding = &event_source.as_raw_fd();
        let mut source = SourceFd(binding);
        let generate_token = self.generate_token();
        let token = mio::Token(generate_token.0);
        self.poll
            .registry()
            .register(&mut source, token, Interest::READABLE)?;
        self.sources
            .insert(generate_token, (event_source, Box::new(callback)));
        Ok(generate_token)
    }

    fn run(&mut self) -> Result<(), CommonError> {
        'outer: loop {
            self.poll.poll(
                &mut self.events,
                Some(std::time::Duration::from_millis(100)),
            )?;
            for event in &mut self.events.iter() {
                if event.is_readable() {
                    let token = event.token();
                    {
                        log::debug!("Timed source with token {:?}", token);
                        let generate_token = Token(token.0);
                        if let Some((source, callback)) = self.sources.get_mut(&generate_token) {
                            callback(source)?;
                        } else if let Some((timer_source, inner_token, callback)) =
                            self.timed_sources.get_mut(&generate_token)
                        {
                            if let Some((source, _)) = self.sources.get_mut(inner_token) {
                                callback(source)?;
                                reset_timer(timer_source)?;
                            }
                        } else {
                            break 'outer;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn add_timer<F>(
        &mut self,
        time_spec: &Itimerspec,
        token: &Token,
        callback: F,
    ) -> Result<Token, CommonError>
    where
        F: FnMut(&mut T) -> Result<i32, CommonError> + 'static,
    {
        let timer_fd = unsafe {
            let fd = libc::timerfd_create(libc::CLOCK_REALTIME, libc::TFD_NONBLOCK);
            let itimer_spec = itimerspec_to_libc(time_spec);

            libc::timerfd_settime(fd, 0, &itimer_spec, std::ptr::null_mut());
            fd
        };
        let mut timer_source = SourceFd(&timer_fd);
        let new_token = self.generate_token();
        let mio_token = mio::Token(new_token.0);
        self.poll
            .registry()
            .register(&mut timer_source, mio_token, Interest::READABLE)?;
        if let Some((_source, _)) = self.sources.get_mut(token) {
            self.timed_sources
                .insert(new_token, (timer_fd, *token, Box::new(callback)));
        }

        Ok(new_token)
    }

    fn add_duration(&mut self, time_spec: &Itimerspec) -> Result<Token, CommonError> {
        let timer_fd = unsafe {
            let fd = libc::timerfd_create(libc::CLOCK_REALTIME, libc::TFD_NONBLOCK);
            let itimer_spec = itimerspec_to_libc(time_spec);
            libc::timerfd_settime(fd, 0, &itimer_spec, std::ptr::null_mut());
            fd
        };

        let mut timer_source = SourceFd(&timer_fd);
        let new_token = self.generate_token();
        let mio_token = mio::Token(new_token.0);

        log::debug!(
            "Added duration {:?} with token mio {:?}  self {:?}",
            time_spec,
            mio_token,
            new_token
        );
        self.poll
            .registry()
            .register(&mut timer_source, mio_token, Interest::READABLE)?;

        Ok(new_token)
    }
}

pub fn reset_timer(timer_raw: &mut RawFd) -> Result<(), CommonError> {
    let timer_spec = &mut libc::itimerspec {
        it_interval: libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
        it_value: libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
    };
    let gettime_result: Result<i32, CommonError> =
        libc_call!(timerfd_gettime(timer_raw.as_raw_fd(), timer_spec));
    gettime_result?;
    let settime_result: Result<i32, CommonError> = libc_call!(timerfd_settime(
        timer_raw.as_raw_fd(),
        0,
        timer_spec,
        timer_spec
    ));
    settime_result?;

    Ok(())
}
