use mio::{unix::SourceFd, Events, Interest, Poll};
use std::{
    collections::HashMap,
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::net::UnixDatagram,
    },
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex, RwLock,
    },
    thread::Thread,
};

use crate::{
    error::CommonError,
    event_loop::{
        itimerspec_to_libc, CallBack, EventLoopTrait, Itimerspec, Source, TimedSource, Token,
    },
    libc_call,
};

pub enum EventLoopMessages<T: Send, U: Send> {
    AddDuration(Itimerspec),
    RegisterTimed((Itimerspec, Token, U)),
    Register(Source<T>),
    Unregister(Token),
    Clean,
    TimedCleanup {
        timer_spec: Itimerspec,
        thread: Thread,
    },
}

/// Event loop specifically tailored for Linux environments.
///
/// This event loop uses epoll (through the mio crate) for I/O multiplexing, and timerfd for timers.
///
/// # Type Parameters
///
/// * `T`: A type that implements `AsRawFd`. This is the type of socket that will be managed by the event loop.
pub struct LinuxEventLoop<T: AsRawFd + Send> {
    poll: Poll,
    events: Events,
    /// A mapping from tokens to registered I/O sources.
    sources: Arc<RwLock<HashMap<Token, Source<T>>>>,
    /// A mapping from tokens to registered timed sources.
    timed_sources: Arc<RwLock<HashMap<Token, TimedSource<T>>>>,
    next_token: AtomicUsize,
    registration_sender: Arc<Mutex<DuplexChannel<T>>>,
    registration_receiver: mpsc::Receiver<EventLoopMessages<T, CallBack<T>>>,
    /// Optional timer specification for an overtime period.
    /// The overtime period removes all timed events, but keeps
    /// listening for readable events
    overtime: Option<Itimerspec>,
    cleanup: Option<Itimerspec>,
    cleanup_token: Option<Token>,
}

impl<T: AsRawFd + Send> LinuxEventLoop<T> {
    /// Returns a sender for the channel used to communicate with the event loop.
    ///
    /// # Returns
    ///
    /// A clone of the `mpsc::Sender` used by the event loop.
    pub fn get_communication_channel(&self) -> Arc<Mutex<DuplexChannel<T>>> {
        self.registration_sender.clone()
    }

    /// Sets a new overtime period for the event loop.
    ///
    /// # Parameters
    ///
    /// * `overtime`: A `Itimerspec` specifying the new overtime period.
    pub fn set_overtime(&mut self, overtime: Itimerspec) {
        self.overtime = Some(overtime);
    }
}

impl<T: AsRawFd + Send + 'static> EventLoopTrait<T> for LinuxEventLoop<T> {
    fn new(event_capacity: usize) -> Result<Self, CommonError> {
        // Create the poll
        let poll = Poll::new()?;
        let events = Events::with_capacity(event_capacity);

        let (registration_sender, registration_receiver) = mpsc::channel();
        let duplex_channel = DuplexChannel::new(registration_sender);
        Ok(Self {
            poll,
            events,
            sources: Arc::new(RwLock::new(HashMap::new())),
            timed_sources: Arc::new(RwLock::new(HashMap::new())),
            next_token: AtomicUsize::new(0),
            registration_sender: Arc::new(Mutex::new(duplex_channel)),
            registration_receiver,
            overtime: Some(Itimerspec {
                it_interval: core::time::Duration::ZERO,
                it_value: core::time::Duration::from_secs(1),
            }),
            cleanup: None,
            cleanup_token: None,
        })
    }

    fn generate_token(&self) -> Token {
        let token = Token(self.next_token.load(std::sync::atomic::Ordering::SeqCst));
        log::debug!("Token: {:?}", token);
        self.next_token
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        log::debug!("Token: {:?}", self.next_token);
        token
    }

    fn run(&mut self) -> Result<(), CommonError> {
        'outer: loop {
            while let Ok(message) = self.registration_receiver.try_recv() {
                match message {
                    EventLoopMessages::Register((event_source, callback)) => {
                        let token = self.register_event_source(event_source, callback)?;
                        self.registration_sender.try_lock()?.set_token(token.0);
                        log::debug!("Registering event source with token {}", token.0);
                    }
                    EventLoopMessages::Unregister(token) => {
                        self.unregister_event_source(token)?;
                    }
                    EventLoopMessages::RegisterTimed((time_spec, token, callback)) => {
                        log::debug!("Registering timedevent source");
                        let timer_token = self.register_timer(&time_spec, &token, callback)?;
                        self.registration_sender
                            .try_lock()?
                            .set_token(timer_token.0);
                    }
                    EventLoopMessages::Clean => {
                        // Unregister all event sources, we do this by closing all file descriptors from the sources and timedsources
                        // the poll is responsible for cleaning up closed Fds
                        self.sources
                            .try_read()?
                            .iter()
                            .for_each(|(_, (source, _))| unsafe {
                                let _ = libc::close(source.as_raw_fd());
                            })
                    }
                    EventLoopMessages::AddDuration(time_spec) => {
                        let token = self.add_duration(&time_spec)?;
                        self.registration_sender.try_lock()?.set_token(token.0);
                    }
                    EventLoopMessages::TimedCleanup { timer_spec, thread } => {
                        log::debug!("Adding cleanup timer");
                        let token = self.add_cleanup(&timer_spec)?;
                        self.registration_sender.try_lock()?.set_token(token.0);
                        thread.unpark();
                    }
                }
            }

            self.poll.poll(
                &mut self.events,
                Some(std::time::Duration::from_millis(100)),
            )?;
            for event in self.events.iter() {
                if event.is_readable() {
                    let token = event.token();
                    log::trace!("Event token {:?}", token);
                    let generate_token = Token(token.0);
                    if let Ok(mut sources) = self.sources.try_write() {
                        if let Ok(mut timed_sources) = self.timed_sources.try_write() {
                            if let Some((source, callback)) = sources.get_mut(&generate_token) {
                                match callback(source, generate_token) {
                                    Ok(_) => (),
                                    Err(e) => {
                                        log::error!(
                                            "An error {:?} has occurred. Closing source",
                                            e
                                        );
                                        drop(sources);
                                        let _ = self.unregister_event_source(generate_token);
                                    }
                                }
                            } else if let Some((timer_source, inner_token, callback)) =
                                timed_sources.get_mut(&generate_token)
                            {
                                log::trace!("Timer event with token {:?}", inner_token);
                                if let Some((source, _)) = sources.get_mut(inner_token) {
                                    callback(source, *inner_token)?;
                                    reset_timer(timer_source)?;
                                }
                            } else {
                                // else only triggers on ungeristered timed events such as TimedCleanup, Overtime and Duration
                                if self.overtime.is_none() {
                                    log::debug!("No overtime");
                                    if self.cleanup.is_none() {
                                        break 'outer;
                                    } else if let Some(cleanup_token) = self.cleanup_token {
                                        drop(timed_sources);
                                        self.unregister_timed_event_source(cleanup_token)?;
                                        self.cleanup_token = None;
                                        continue 'outer;
                                    }
                                }

                                let tokens: Vec<Token> =
                                    timed_sources.iter().map(|(token, _)| *token).collect();
                                drop(timed_sources);
                                // Unregister all timed events
                                tokens.iter().for_each(|token| {
                                    let _ = self.unregister_timed_event_source(*token);
                                });

                                log::debug!("Entering Overtime {:?}", self.overtime);
                                let overtime = self.overtime.take().expect("No overtime");
                                self.cleanup_token = Some(self.add_duration(&overtime)?);
                            }
                        }
                    }
                }
            }
            // Check if there are any sources with closed file descriptors and deregister them
            let sources_clone = self.sources.clone();
            let sources_reference = sources_clone.try_read()?;
            let dead_tokens = sources_reference.iter().filter_map(|(token, (source, _))| {
                let fd = source.as_raw_fd();

                if !is_fd_open(&fd) {
                    return Some(*token);
                }
                None
            });
            dead_tokens.for_each(|token| {
                let _ = self.unregister_event_source(token);
                let _ = self.unregister_timed_event_source(token);
            });
        }

        Ok(())
    }

    fn register_timer(
        &self,
        time_spec: &Itimerspec,
        token: &Token,
        callback: CallBack<T>,
    ) -> Result<Token, CommonError> {
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
        if let Some((_source, _)) = self.sources.try_write()?.get_mut(token) {
            self.timed_sources
                .try_write()?
                .insert(new_token, (timer_fd, *token, Box::new(callback)));
        }
        Ok(new_token)
    }

    fn add_duration(&self, time_spec: &Itimerspec) -> Result<Token, CommonError> {
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

        Ok(new_token)
    }

    fn register_event_source(
        &self,
        event_source: T,
        callback: CallBack<T>,
    ) -> Result<Token, CommonError> {
        let binding = &event_source.as_raw_fd();
        let mut source = SourceFd(binding);
        let generate_token = self.generate_token();
        let token = mio::Token(generate_token.0);
        self.poll
            .registry()
            .register(&mut source, token, Interest::READABLE)?;
        self.sources
            .try_write()
            .unwrap()
            .insert(generate_token, (event_source, Box::new(callback)));
        Ok(generate_token)
    }

    fn unregister_event_source(&self, token: Token) -> Result<(), CommonError> {
        if let Ok(mut sources) = self.sources.try_write() {
            if let Some((event_source, _)) = sources.remove(&token) {
                let raw_fd = &event_source.as_raw_fd();
                let mut source_fd = SourceFd(raw_fd);
                self.poll
                    .registry()
                    .deregister(&mut source_fd)
                    .map_err(|e| {
                        CommonError::from(format!("Failed to deregister event source: {}", e))
                    })?;
            } else {
                return Err(CommonError::from(
                    "Failed to unregister event source: token not found".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn unregister_timed_event_source(&self, token: Token) -> Result<(), CommonError> {
        if let Some((timer_fd, _event_token, _)) = self.timed_sources.try_write()?.remove(&token) {
            log::debug!("Unregistering timed event with token {:?}", token);
            // Unregister timer_fd
            let mut timer_source = SourceFd(&timer_fd);
            self.poll
                .registry()
                .deregister(&mut timer_source)
                .map_err(|e| {
                    let error_message = format!("Failed to deregister timed event source: {}", e);
                    log::error!("{}", error_message);
                    CommonError::from(error_message)
                })?;
        } else {
            return Err(CommonError::from(
                "Failed to unregister timed event source: token not found".to_string(),
            ));
        }
        Ok(())
    }

    fn add_cleanup(&mut self, time_spec: &Itimerspec) -> Result<Token, CommonError> {
        self.cleanup = Some(time_spec.to_owned());
        let timer_fd = unsafe {
            let fd = libc::timerfd_create(libc::CLOCK_REALTIME, libc::TFD_NONBLOCK);
            let itimer_spec = itimerspec_to_libc(time_spec);
            let res = libc::timerfd_settime(fd, 0, &itimer_spec, std::ptr::null_mut());
            log::debug!("Timerfd settime result: {}", res);
            fd
        };

        let mut timer_source = SourceFd(&timer_fd);
        let new_token = self.generate_token();
        let mio_token = mio::Token(new_token.0);
        self.poll
            .registry()
            .register(&mut timer_source, mio_token, Interest::READABLE)?;
        log::debug!("Registered cleanup");
        self.cleanup_token = Some(new_token);
        Ok(new_token)
    }
}

/// Resets the specified timer.
///
/// # Parameters
///
/// * `timer_raw`: A mutable reference to the raw file descriptor of the timer to reset.
///
/// # Returns
///
/// A `Result` that is `Ok(())` if the timer was successfully reset, and `Err(CommonError)` otherwise.
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

/// Creates a new non-blocking Unix datagram socket.
///
/// # Returns
///
/// A `Result` that is `Ok(UnixDatagram)` if the socket was successfully created, and `Err(CommonError)` otherwise.
pub fn create_non_blocking_unix_datagram() -> Result<UnixDatagram, CommonError> {
    let socket_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };
    if socket_fd < 0 {
        return Err(CommonError::Io(std::io::Error::last_os_error()));
    }

    let flags = unsafe { libc::fcntl(socket_fd, libc::F_GETFL) };
    if flags < 0 {
        let _ = unsafe { libc::close(socket_fd) };
        return Err(CommonError::Io(std::io::Error::last_os_error()));
    }

    let result = unsafe { libc::fcntl(socket_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        let _ = unsafe { libc::close(socket_fd) };
        return Err(CommonError::Io(std::io::Error::last_os_error()));
    }

    Ok(unsafe { UnixDatagram::from_raw_fd(socket_fd) })
}

/// Checks if file descriptor is open or closed returning a boolean value
fn is_fd_open<T: AsRawFd>(file: &T) -> bool {
    let fd = file.as_raw_fd();
    let res = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    unsafe { !(res == -1 && *libc::__errno_location() == libc::EBADF) }
}

pub struct DuplexChannel<T>
where
    T: Send,           // Ensure T is Send
    CallBack<T>: Send, // Ensure Callback<T> is Send
{
    sender: mpsc::Sender<EventLoopMessages<T, CallBack<T>>>,
    token: Arc<AtomicUsize>, // Stores the inner value of Token(usize)
    error: Arc<Mutex<Option<CommonError>>>, // For storing error state
}

impl<T> Clone for DuplexChannel<T>
where
    T: Send,
    CallBack<T>: Send,
{
    fn clone(&self) -> Self {
        DuplexChannel {
            sender: self.sender.clone(),
            token: self.token.clone(),
            error: self.error.clone(),
        }
    }
}

impl<T> DuplexChannel<T>
where
    T: Send,
    CallBack<T>: Send,
{
    // Initialize the DuplexChannel
    pub fn new(sender: mpsc::Sender<EventLoopMessages<T, CallBack<T>>>) -> Self {
        DuplexChannel {
            sender,
            token: Arc::new(AtomicUsize::new(usize::MAX)), // Invalid token state
            error: Arc::new(Mutex::new(None)),
        }
    }

    // Send a message to the event loop
    pub fn send(&self, message: EventLoopMessages<T, CallBack<T>>) -> Result<(), CommonError> {
        self.sender.send(message).map_err(CommonError::from)
    }

    // Called by event loop to set the token value
    pub fn set_token(&self, token_value: usize) {
        self.token.store(token_value, Ordering::SeqCst);
    }

    // Retrieve the token, if available
    pub fn get_token(&self) -> Result<Token, CommonError> {
        let token_value = self.token.load(Ordering::SeqCst);
        if token_value != usize::MAX {
            // Check if token is valid
            let token = Token(token_value);
            Ok(token)
        } else {
            // Retrieve and clear error state
            let mut lock = self.error.lock().unwrap();
            if let Some(err) = lock.take() {
                Err(err)
            } else {
                Err(CommonError::Generic("Invalid token".to_string()))
            }
        }
    }

    // Update error state
    pub fn set_error(&self, error: CommonError) {
        let mut lock = self.error.lock().unwrap();
        *lock = Some(error);
    }
}
