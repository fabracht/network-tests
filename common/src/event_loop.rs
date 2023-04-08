use crate::{error::CommonError, socket::Socket};
use core::time::Duration;
use std::os::fd::{AsRawFd, RawFd};

pub type Sources<T> = (T, Box<dyn FnMut(&mut T) -> Result<i32, CommonError>>);
pub type TimedSources<T> = (
    RawFd,
    Token,
    Box<dyn FnMut(&mut T) -> Result<i32, CommonError>>,
);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub usize);

impl From<Token> for usize {
    fn from(val: Token) -> usize {
        val.0
    }
}

#[derive(Debug)]
pub struct Itimerspec {
    pub it_interval: Duration,
    pub it_value: Duration,
}

impl Itimerspec {
    pub fn duration_millis(&self) -> isize {
        // Calculate the duration in milliseconds based on it_value
        let seconds_in_millis = self.it_value.as_secs() * 1000;
        let nanos_in_millis = self.it_value.subsec_nanos() as u64 / 1_000_000;

        (seconds_in_millis + nanos_in_millis) as isize
    }

    pub fn duration_micros(&self) -> isize {
        // Calculate the duration in microseconds based on it_value
        self.duration_millis() * 1000
    }
}

pub trait EventLoopTrait<T: AsRawFd + for<'a> Socket<'a, T>> {
    fn new(event_capacity: usize) -> Self;
    fn generate_token(&mut self) -> Token;
    fn register_event_source<F>(
        &mut self,
        event_source: T,
        callback: F,
    ) -> Result<Token, CommonError>
    where
        F: FnMut(&mut T) -> Result<i32, CommonError> + 'static;
    fn run(&mut self) -> Result<(), CommonError>;
    fn add_duration(&mut self, time_spec: &Itimerspec) -> Result<Token, CommonError>;
    fn add_timer<F>(
        &mut self,
        time_spec: &Itimerspec,
        token: &Token,
        callback: F,
    ) -> Result<Token, CommonError>
    where
        F: FnMut(&mut T) -> Result<i32, CommonError> + 'static;
}

#[cfg(target_os = "linux")]
pub fn itimerspec_to_libc(itimer: &Itimerspec) -> libc::itimerspec {
    libc::itimerspec {
        it_interval: libc::timespec {
            tv_sec: itimer.it_interval.as_secs() as libc::time_t,
            tv_nsec: itimer.it_interval.subsec_nanos() as libc::c_long,
        },
        it_value: libc::timespec {
            tv_sec: itimer.it_value.as_secs() as libc::time_t,
            tv_nsec: itimer.it_value.subsec_nanos() as libc::c_long,
        },
    }
}
