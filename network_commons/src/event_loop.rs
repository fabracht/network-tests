use crate::error::CommonError;
use core::time::Duration;
use std::os::fd::{AsRawFd, RawFd};

pub type CallBack<T> = Box<dyn FnMut(&mut T, Token) -> Result<isize, CommonError> + Send + 'static>;
pub type Source<T> = (T, CallBack<T>);
pub type SourceCollection<T> = (T, Vec<CallBack<T>>);
pub type TimedSource<T> = (RawFd, Token, CallBack<T>);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub usize);

impl From<Token> for usize {
    fn from(val: Token) -> usize {
        val.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
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

/// `EventLoopTrait` defines the behavior for an event loop.
///
/// This trait is implemented by types that can be used to manage and
/// control event-driven systems, such as network services.
pub trait EventLoopTrait<T: AsRawFd> {
    /// Creates a new event loop instance with the given capacity.
    ///
    /// # Errors
    /// Returns `CommonError` if the creation fails.
    fn new(event_capacity: usize) -> Result<Self, CommonError>
    where
        Self: Sized;
    /// Generates a new unique token.
    ///
    /// Tokens are used to identify registered event sources.
    fn generate_token(&self) -> Token;
    /// Registers an event source to the event loop.
    ///
    /// The `event_source` is the entity that can produce events.
    /// The `callback` is a function that will be called when an event
    /// is received for the registered `event_source`.
    ///
    /// # Errors
    /// Returns `CommonError` if the registration fails.
    fn register_event_source(
        &self,
        event_source: T,
        callback: CallBack<T>,
    ) -> Result<Token, CommonError>;
    /// Unregisters the specified event source from the event loop.
    ///
    /// The `token` identifies the event source to be unregistered.
    ///
    /// # Errors
    /// Returns `CommonError` if the unregistration fails.
    fn unregister_event_source(&self, token: Token) -> Result<(), CommonError>;

    /// Unregisters a timed event source from the event loop.
    ///
    /// The `token` identifies the timed event source to be unregistered.
    ///
    /// # Errors
    /// Returns `CommonError` if the unregistration fails.
    fn unregister_timed_event_source(&self, token: Token) -> Result<(), CommonError>;

    /// Runs the event loop.
    ///
    /// The event loop will keep running until an error occurs or it is manually stopped.
    ///
    /// # Errors
    /// Returns `CommonError` if running the event loop fails.
    fn run(&mut self) -> Result<(), CommonError>;

    /// Adds a timed event to the event loop.
    ///
    /// The `time_spec` specifies when the event should be triggered.
    ///
    /// # Errors
    /// Returns `CommonError` if adding the timed event fails.
    fn add_duration(&self, time_spec: &Itimerspec) -> Result<Token, CommonError>;

    /// Adds a timed event that clears all registered events to the event loop.
    ///
    /// The `time_spec` specifies when the event should be triggered.
    ///
    /// # Errors
    /// Returns `CommonError` if adding the timed event fails.
    fn add_cleanup(&mut self, time_spec: &Itimerspec) -> Result<Token, CommonError>;

    /// Adds a timer to the event loop.
    ///
    /// The `time_spec` specifies when the timer should be triggered.
    /// The `token` is the identifier for the timer.
    /// The `callback` is a function that will be called when the timer is triggered.
    ///
    /// # Errors
    /// Returns `CommonError` if adding the timer fails.
    fn register_timer(
        &self,
        time_spec: &Itimerspec,
        token: &Token,
        callback: CallBack<T>,
    ) -> Result<Token, CommonError>;
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
