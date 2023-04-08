use std::{
    collections::HashMap,
    os::fd::{AsRawFd, RawFd},
};

use libc::{EVFILT_READ, EVFILT_TIMER, EV_ADD, EV_ENABLE, EV_ONESHOT, NOTE_USECONDS};

use crate::{
    error::CommonError,
    event_loop::{EventLoopTrait, Itimerspec, Sources, TimedSources, Token},
    libc_call,
    socket::Socket,
};

pub struct MacOSEventLoop<T: AsRawFd + for<'a> Socket<'a, T>> {
    kqueue: RawFd,
    events: Vec<libc::kevent>,
    sources: HashMap<Token, Sources<T>>,
    timed_sources: HashMap<Token, TimedSources<T>>,
    next_token: usize,
}

impl<T: AsRawFd + for<'a> Socket<'a, T>> EventLoopTrait<T> for MacOSEventLoop<T> {
    fn new(event_capacity: usize) -> Self {
        let kqueue = unsafe { libc::kqueue() };
        if kqueue < 0 {
            panic!("Failed to create kqueue");
        }

        let events = Vec::with_capacity(event_capacity);

        Self {
            kqueue,
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
        let token = self.generate_token();

        let kevent = libc::kevent {
            ident: event_source.as_raw_fd() as _,
            filter: EVFILT_READ,
            flags: EV_ADD | EV_ENABLE,
            fflags: 0,
            data: 0,
            udata: token.0 as *mut _,
        };

        let result = unsafe {
            libc::kevent(
                self.kqueue,
                &kevent,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if result < 0 {
            return Err(CommonError::KeventRegistrationError(
                std::io::Error::last_os_error(),
            ));
        }

        self.sources
            .insert(token, (event_source, Box::new(callback)));
        Ok(token)
    }

    fn run(&mut self) -> Result<(), CommonError> {
        'outer: loop {
            log::info!("Running event loop");
            let nevents_result: Result<i32, CommonError> = libc_call!(kevent(
                self.kqueue,
                std::ptr::null(),
                0,
                self.events.as_mut_ptr(),
                self.events.len() as i32,
                std::ptr::null()
            ));
            let nevents = nevents_result?;
            for i in 0..nevents {
                let event = &self.events[i as usize];
                if event.filter == EVFILT_TIMER {
                    let token = Token(event.ident as usize);
                    if let Some((timer_source, inner_token, callback)) =
                        self.timed_sources.get_mut(&token)
                    {
                        if let Some((source, _)) = self.sources.get_mut(inner_token) {
                            callback(source)?;
                            reset_timer(self.kqueue, timer_source)?;
                        }
                    } else {
                        break 'outer;
                    }
                }
            }
        }

        Ok(())
    }

    fn add_duration(&mut self, time_spec: &Itimerspec) -> Result<Token, CommonError> {
        let token = self.generate_token();

        let kevent = libc::kevent {
            ident: token.0 as _,
            filter: EVFILT_TIMER,
            flags: EV_ADD | EV_ENABLE | EV_ONESHOT,
            fflags: NOTE_USECONDS,
            data: time_spec.duration_millis() * 1000,
            udata: token.0 as *mut _,
        };

        let result = unsafe {
            libc::kevent(
                self.kqueue,
                &kevent,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if result < 0 {
            return Err(CommonError::KeventRegistrationError(
                std::io::Error::last_os_error(),
            ));
        }

        Ok(token)
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
        let timer_token = self.generate_token();
        let timer_duration_micros = time_spec.duration_micros();

        let kev = libc::kevent {
            ident: timer_token.0,
            filter: EVFILT_TIMER,
            flags: EV_ADD | EV_ENABLE,
            fflags: libc::NOTE_USECONDS,
            data: timer_duration_micros,
            udata: 0 as *mut _,
        };

        let kevent_result: Result<i32, CommonError> = libc_call!(kevent(
            self.kqueue,
            &kev as *const libc::kevent,
            1,
            std::ptr::null_mut(),
            0,
            std::ptr::null()
        ));

        kevent_result?;

        if let Some((event_source, _)) = self.sources.get(token) {
            self.timed_sources.insert(
                timer_token,
                (event_source.as_raw_fd(), *token, Box::new(callback)),
            );
        } else {
            return Err(CommonError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Event source not found",
            )));
        }

        Ok(timer_token)
    }
}

fn reset_timer(kqueue: RawFd, timer_source: &RawFd) -> Result<(), CommonError> {
    let kev = libc::kevent {
        ident: timer_source.to_owned() as usize,
        filter: EVFILT_TIMER,
        flags: EV_ADD | EV_ENABLE,
        fflags: 0,
        data: 0,
        udata: 0 as *mut _,
    };

    let kevent_result: Result<i32, CommonError> = libc_call!(kevent(
        kqueue,
        &kev as *const libc::kevent,
        1,
        std::ptr::null_mut(),
        0,
        std::ptr::null()
    ));

    kevent_result.map_err(|e| {
        CommonError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to reset timer: {}", e),
        ))
    })?;

    Ok(())
}
