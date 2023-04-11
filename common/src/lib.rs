//! This crate provides a library for implementing the Two-Way Active Measurement Protocol (TWAMP),
//! which is a protocol for measuring the performance of IP networks. The library provides various
//! abstractions and utilities for customizing and running TWAMP tests.
//!
//! # Usage
//!
//! To use this crate, add the following to your `Cargo.toml` file:
//!
//! ```toml
//! [dependencies]
//! twamp = "*"
//! ```

use error::CommonError;

pub mod error;

#[cfg(target_os = "linux")]
pub mod epoll_loop;
pub mod event_loop;
pub mod host;
#[cfg(target_os = "macos")]
pub mod kevent_loop;
pub mod message;
pub mod session;
pub mod socket;
pub mod statistics;
pub mod time;
/// A trait representing a TWAMP strategy, which is an abstraction for TWAMP implementors to
/// customize the runtime of the test. Implementors of this trait provide a custom implementation
/// of the `execute` method, which is called to execute the TWAMP test with the specified
/// configuration.
///
/// # Type Parameters
///
/// - `R`: The type of result that is returned by the `execute` method.
/// - `E`: The type of error that can be returned by the `execute` method.
pub trait Strategy<R: TestResult, E: std::error::Error> {
    /// Executes the TWAMP test with the specified configuration, using the custom implementation
    /// provided by the implementor of this trait.
    ///
    /// # Returns
    ///
    /// A `Result` that contains the result of the TWAMP test, or an error if the test failed.
    fn execute(&mut self) -> std::result::Result<R, E>;
}

pub trait TestResult: Send {
    fn status(&self) -> Result<(), CommonError> {
        Ok(())
    }
}

#[macro_export]
macro_rules! assert_approx_eq {
    ($a:expr, $b:expr, $epsilon:expr) => {{
        let (a, b, eps) = (&$a, &$b, &$epsilon);
        assert!(
            (*a - *b).abs() < *eps,
            "{:?} is not approximately equal to {:?} (epsilon = {:?})",
            *a,
            *b,
            *eps
        );
    }};
}

#[macro_export]
macro_rules! libc_call {
    ($name:ident($($arg_name:expr), *)) => (unsafe {
        let result = libc::$name($($arg_name),*) ;
        if result == -1 {
            let err = std::io::Error::last_os_error();
            let err_msg = std::ffi::CStr::from_ptr(libc::strerror(err.raw_os_error().unwrap()));
            return Err(std::io::Error::new(err.kind(), err_msg.to_string_lossy().into_owned()).into());
        }
        std::result::Result::Ok(result)
    })
}
