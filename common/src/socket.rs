use super::error::CommonError;
use crate::time::DateTime;
use message_macro::BeBytes;
use std::net::SocketAddr;
use std::os::fd::{AsRawFd, RawFd};

/// A trait representing a socket that can send and receive data.
pub trait Socket<'a, T: AsRawFd> {
    /// Creates a new instance of the socket from the given raw file descriptor.
    fn from_raw_fd(fd: RawFd) -> T;

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
}
