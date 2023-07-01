use super::error::CommonError;
use crate::libc_call;
use crate::time::DateTime;
use message_macro::BeBytes;
use std::net::SocketAddr;
use std::os::fd::{AsRawFd, RawFd};

/// A trait representing a socket that can send and receive data.
pub trait Socket<'a, T: AsRawFd>: Sized + AsRawFd {
    /// Creates a new instance of the socket from the given raw file descriptor.
    ///
    /// # Safety
    ///
    /// When implementing this, you have to make sure the file descriptor is valid
    unsafe fn from_raw_fd(fd: RawFd) -> T;

    /// Sends the given message over the socket.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send, which must implement the `BeBytes` trait for big-endian byte order.
    ///
    /// # Returns
    ///
    /// A `Result` that contains the number of bytes sent and the DateTime when the message was sent, or a `CommonError` if an error occurred.
    fn send(&self, message: impl BeBytes) -> Result<(usize, DateTime), CommonError>;

    /// Sends the given message to the specified socket address.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to send the message to.
    /// * `message` - The message to send, which must implement the `BeBytes` trait for big-endian byte order.
    ///
    /// # Returns
    ///
    /// A `Result` that contains the number of bytes sent and the DateTime when the message was sent, or a `CommonError` if an error occurred.
    fn send_to(
        &self,
        address: &SocketAddr,
        message: impl BeBytes,
    ) -> Result<(usize, DateTime), CommonError>;

    /// Receives data from the socket into the given buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to receive the data into.
    ///
    /// # Returns
    ///
    /// A `Result` that contains the number of bytes received and the DateTime when the message was received, or a `CommonError` if an error occurred.
    fn receive(&self, buffer: &mut [u8]) -> Result<(usize, DateTime), CommonError>;

    /// Receives data from the socket into the given buffer, along with the address of the sender.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to receive the data into.
    ///
    /// # Returns
    ///
    /// A `Result` that contains the number of bytes received, the sender's address, and the DateTime when the message was received, or a `CommonError` if an error occurred.
    fn receive_from(&self, buffer: &mut [u8])
        -> Result<(usize, SocketAddr, DateTime), CommonError>;

    fn set_socket_options(
        &mut self,
        level: i32,
        name: i32,
        value: Option<i32>,
    ) -> Result<i32, CommonError> {
        let res = libc_call!(setsockopt(
            self.as_raw_fd(),
            level,
            name,
            &value.unwrap_or(0) as *const std::ffi::c_int as *const std::ffi::c_void,
            std::mem::size_of_val(&value) as libc::socklen_t
        ))
        .map_err(CommonError::Io)?;
        log::debug!("setsockopt:level {}, name {}, res {}", level, name, res);
        Ok(res)
    }

    fn set_fcntl_options(&self) -> Result<(), CommonError> {
        // Get current flags
        let flags = libc_call!(fcntl(self.as_raw_fd(), libc::F_GETFL)).map_err(CommonError::Io)?;

        // Add O_NONBLOCK and O_CLOEXEC to the flags
        let new_flags = flags | libc::O_NONBLOCK | libc::O_CLOEXEC;

        // Set the new flags
        let _res = libc_call!(fcntl(self.as_raw_fd(), libc::F_SETFL, new_flags))
            .map_err(CommonError::Io)?;

        Ok(())
    }

    fn set_timestamping_options(&mut self) -> Result<i32, CommonError> {
        let value = libc::SOF_TIMESTAMPING_SOFTWARE
            | libc::SOF_TIMESTAMPING_RX_SOFTWARE
            | libc::SOF_TIMESTAMPING_TX_SOFTWARE;
        self.set_socket_options(libc::SOL_SOCKET, libc::SO_TIMESTAMPING, Some(value as i32))
    }
}
