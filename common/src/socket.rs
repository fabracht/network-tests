use message_macro::BeBytes;

use std::net::SocketAddr;
use std::os::fd::{AsRawFd, RawFd};

use crate::time::DateTime;

use super::error::CommonError;

pub trait Socket<'a, T: AsRawFd> {
    fn from_raw_fd(fd: RawFd) -> T;
    fn send(&self, message: impl BeBytes) -> Result<(usize, DateTime), CommonError>;
    fn send_to(
        &self,
        address: &SocketAddr,
        message: impl BeBytes,
    ) -> Result<(usize, DateTime), CommonError>;
    fn receive(&self, buffer: &mut [u8]) -> Result<(usize, DateTime), CommonError>;
    fn receive_from(&self, buffer: &mut [u8])
        -> Result<(usize, SocketAddr, DateTime), CommonError>;
}
