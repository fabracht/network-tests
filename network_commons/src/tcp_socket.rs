use bebytes::BeBytes;
use libc::MSG_NOSIGNAL;

use crate::{
    socket::{socketaddr_to_sockaddr, storage_to_socket_addr, Socket},
    time::DateTime,
    CommonError,
};
use core::ops::Deref;

use std::{
    io,
    net::SocketAddr,
    os::fd::{AsRawFd, RawFd},
};

pub enum SocketError {
    BindFailed(io::Error),
    ListenFailed(io::Error),
    AcceptFailed(io::Error),
}

/// A TCP socket wrapper that includes the raw file descriptor.
///
/// This structure is intended to wrap the raw file descriptor provided by a
/// TCP socket and includes some common socket operations like `bind`, `listen`,
/// `accept`, and `connect`. It also provides timestamped send and receive operations.
///
/// ## Safety
///
/// This structure performs raw system calls via the libc crate. Incorrect use could lead
/// to system errors. Ensure the correct use of these system calls in accordance with
/// POSIX standards.
pub struct TimestampedTcpSocket {
    inner: RawFd,
}

impl Drop for TimestampedTcpSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.inner) };
    }
}

impl AsRawFd for TimestampedTcpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.inner
    }
}

impl From<&mut i32> for TimestampedTcpSocket {
    fn from(value: &mut i32) -> Self {
        Self::new(value.as_raw_fd())
    }
}

impl Deref for TimestampedTcpSocket {
    type Target = RawFd;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TimestampedTcpSocket {
    /// Create a new instance of `TimestampedTcpSocket` from a raw file descriptor.
    ///
    /// This method sets the `SO_REUSEADDR` option on the socket to allow the reuse
    /// of local addresses.
    ///
    /// ## Safety
    ///
    /// The provided file descriptor should be valid and correspond to a socket.
    pub fn new(socket: RawFd) -> Self {
        unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_REUSEADDR,
                1 as *const _,
                std::mem::size_of::<i32>() as u32,
            );
        }
        Self { inner: socket }
    }

    /// Binds the socket to a specific address.
    ///
    /// The socket will be available for incoming connection attempts on the
    /// specified `addr`.
    ///
    /// # Errors
    ///
    /// This method returns an error if the socket cannot be bound to the provided
    /// address.
    pub fn bind(addr: &SocketAddr) -> Result<Self, CommonError> {
        let socket_fd = match addr {
            SocketAddr::V4(_) => unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) },
            SocketAddr::V6(_) => unsafe { libc::socket(libc::AF_INET6, libc::SOCK_STREAM, 0) },
        };

        if socket_fd < 0 {
            return Err(CommonError::SocketCreateFailed(io::Error::last_os_error()));
        }
        let (sock_addr, sock_addr_len) = socketaddr_to_sockaddr(addr);
        let sock_addr_ptr = &sock_addr as *const _;

        if unsafe { libc::bind(socket_fd, sock_addr_ptr, sock_addr_len) } < 0 {
            return Err(CommonError::SocketBindFailed(io::Error::last_os_error()));
        }

        Ok(TimestampedTcpSocket { inner: socket_fd })
    }

    /// Listen for incoming connections.
    ///
    /// The `backlog` parameter defines the maximum number of pending connections.
    ///
    /// # Errors
    ///
    /// This method returns an error if the socket cannot be set to listen mode.
    pub fn listen(&self, backlog: i32) -> Result<(), CommonError> {
        if unsafe { libc::listen(self.inner, backlog) } < 0 {
            Err(CommonError::SocketListenFailed(io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    /// Accept a new incoming connection attempt.
    ///
    /// This method blocks until a connection attempt is made to the socket.
    ///
    /// # Returns
    ///
    /// This method returns a new `TimestampedTcpSocket` for the incoming connection
    /// and the address of the peer socket.
    ///
    /// # Errors
    ///
    /// This method returns an error if an incoming connection cannot be accepted.
    pub fn accept(&self) -> Result<(TimestampedTcpSocket, SocketAddr), CommonError> {
        let mut addr_storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut addr_len = std::mem::size_of_val(&addr_storage) as libc::socklen_t;

        let new_socket_fd = unsafe {
            libc::accept(
                self.inner,
                &mut addr_storage as *mut libc::sockaddr_storage as *mut libc::sockaddr,
                &mut addr_len,
            )
        };

        if new_socket_fd < 0 {
            return Err(CommonError::SocketAcceptFailed(io::Error::last_os_error()));
        }
        let client_addr = storage_to_socket_addr(&addr_storage)?;
        Ok((
            TimestampedTcpSocket {
                inner: new_socket_fd,
            },
            client_addr,
        ))
    }

    /// Connect to a remote socket at the provided address.
    ///
    /// This method blocks until the connection is established.
    ///
    /// # Errors
    ///
    /// This method returns an error if the connection attempt fails.
    pub fn connect(&mut self, addr: SocketAddr) -> Result<i32, CommonError> {
        let socket_fd = self.inner;
        if socket_fd < 0 {
            return Err(CommonError::SocketCreateFailed(io::Error::last_os_error()));
        }
        let (sock_addr, sock_addr_len) = socketaddr_to_sockaddr(&addr);
        let sock_addr_ptr = &sock_addr as *const _;
        let result = unsafe { libc::connect(socket_fd, sock_addr_ptr, sock_addr_len) };
        log::debug!("Connect result: {}", result);
        if result < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(socket_fd) };
            return Err(CommonError::SocketConnectFailed(err));
        }

        Ok(result)
    }
}

impl Socket<TimestampedTcpSocket> for TimestampedTcpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> TimestampedTcpSocket {
        Self { inner: fd }
    }

    fn send(&self, message: impl BeBytes) -> Result<(isize, DateTime), CommonError> {
        // Convert the message to a byte array
        let bytes = message.to_be_bytes();

        // Get the current timestamp
        let timestamp = DateTime::utc_now();
        // Send the data using the libc send function
        let result = unsafe {
            libc::send(
                self.inner,
                bytes.as_ptr() as *const libc::c_void,
                bytes.len(),
                MSG_NOSIGNAL,
            )
        };

        // Check if there was an error during the send operation
        if result < 0 {
            let error = io::Error::last_os_error();
            return Err(CommonError::from(error));
        }

        // Return the number of bytes sent and the timestamp
        Ok((result, timestamp))
    }

    fn send_to(
        &self,
        _address: &SocketAddr,
        message: impl BeBytes,
    ) -> Result<(isize, crate::time::DateTime), CommonError> {
        // Use the send method to send the data
        self.send(message)
    }

    fn receive(&self, buffer: &mut [u8]) -> Result<(isize, DateTime), CommonError> {
        // Get the current timestamp
        let timestamp = DateTime::utc_now();

        // Receive data using the libc recv function
        let result = unsafe {
            libc::recv(
                self.inner,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
                MSG_NOSIGNAL,
            )
        };

        // Check if there was an error during the receive operation
        if result < 0 {
            let error = io::Error::last_os_error();
            return Err(CommonError::from(error));
        }

        // Return the number of bytes received and the timestamp
        Ok((result, timestamp))
    }

    fn receive_from(
        &self,
        buffer: &mut [u8],
    ) -> Result<(isize, SocketAddr, DateTime), CommonError> {
        let (result, timestamp) = self.receive(buffer)?;

        let mut addr_storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut addr_len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;

        if unsafe {
            libc::getpeername(
                self.inner,
                &mut addr_storage as *mut _ as *mut _,
                &mut addr_len,
            )
        } == -1
        {
            return Err(CommonError::SocketGetPeerName(io::Error::last_os_error()));
        }
        let peer_address = storage_to_socket_addr(&addr_storage)?;
        Ok((result, peer_address, timestamp))
    }
}
