use libc::{sockaddr, sockaddr_in, sockaddr_in6, socklen_t, AF_INET, AF_INET6, MSG_NOSIGNAL};
use message_macro::BeBytes;

use crate::{socket::Socket, time::DateTime, CommonError};
use core::ops::Deref;

use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
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

        let (sock_addr, sock_addr_len) = match addr {
            SocketAddr::V4(a) => {
                let ip_octets = a.ip().octets();
                let in_addr = libc::in_addr {
                    s_addr: u32::from_be_bytes(ip_octets),
                };
                let sockaddr = sockaddr_in {
                    sin_family: AF_INET as libc::sa_family_t,
                    sin_port: a.port().to_be(),
                    sin_addr: in_addr,
                    sin_zero: [0; 8],
                };
                (
                    &sockaddr as *const sockaddr_in as *const sockaddr,
                    std::mem::size_of_val(&sockaddr) as socklen_t,
                )
            }
            SocketAddr::V6(a) => {
                let ip_octets = a.ip().octets();
                let mut in6_addr = libc::in6_addr { s6_addr: [0; 16] };
                in6_addr.s6_addr.copy_from_slice(&ip_octets);
                let sockaddr = sockaddr_in6 {
                    sin6_family: AF_INET6 as libc::sa_family_t,
                    sin6_port: a.port().to_be(),
                    sin6_addr: in6_addr,
                    sin6_flowinfo: a.flowinfo(),
                    sin6_scope_id: a.scope_id(),
                };
                (
                    &sockaddr as *const sockaddr_in6 as *const sockaddr,
                    std::mem::size_of_val(&sockaddr) as socklen_t,
                )
            }
        };

        if unsafe { libc::bind(socket_fd, sock_addr, sock_addr_len) } < 0 {
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

        let client_addr = match addr_storage.ss_family as libc::c_int {
            AF_INET => {
                let sockaddr: *const sockaddr_in = &addr_storage as *const _ as *const sockaddr_in;
                let sockaddr: &sockaddr_in = unsafe { &*sockaddr };
                let ip = Ipv4Addr::from(sockaddr.sin_addr.s_addr.to_le_bytes());
                let port = u16::from_be(sockaddr.sin_port);
                SocketAddr::V4(std::net::SocketAddrV4::new(ip, port))
            }
            AF_INET6 => {
                let sockaddr: *const sockaddr_in6 =
                    &addr_storage as *const _ as *const sockaddr_in6;
                let sockaddr: &sockaddr_in6 = unsafe { &*sockaddr };
                let ip = Ipv6Addr::from(sockaddr.sin6_addr.s6_addr);
                let port = u16::from_be(sockaddr.sin6_port);
                let flowinfo = sockaddr.sin6_flowinfo;
                let scope_id = sockaddr.sin6_scope_id;
                SocketAddr::V6(std::net::SocketAddrV6::new(ip, port, flowinfo, scope_id))
            }
            _ => return Err(CommonError::UnknownAddressFamily),
        };
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
    pub fn connect(addr: SocketAddr) -> Result<TimestampedTcpSocket, CommonError> {
        let socket_fd = match addr {
            SocketAddr::V4(_) => unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) },
            SocketAddr::V6(_) => unsafe { libc::socket(libc::AF_INET6, libc::SOCK_STREAM, 0) },
        };

        if socket_fd < 0 {
            return Err(CommonError::SocketCreateFailed(io::Error::last_os_error()));
        }

        let (sock_addr, sock_addr_len) = match addr {
            SocketAddr::V4(a) => {
                let ip_octets = a.ip().octets();
                let in_addr = libc::in_addr {
                    s_addr: u32::from_be_bytes(ip_octets),
                };
                let sockaddr = sockaddr_in {
                    sin_family: AF_INET as libc::sa_family_t,
                    sin_port: a.port().to_be(),
                    sin_addr: in_addr,
                    sin_zero: [0; 8],
                };
                (
                    &sockaddr as *const sockaddr_in as *const sockaddr,
                    std::mem::size_of_val(&sockaddr) as socklen_t,
                )
            }
            SocketAddr::V6(a) => {
                let ip_octets = a.ip().octets();
                let mut in6_addr = libc::in6_addr { s6_addr: [0; 16] };
                in6_addr.s6_addr.copy_from_slice(&ip_octets);
                let sockaddr = sockaddr_in6 {
                    sin6_family: AF_INET6 as libc::sa_family_t,
                    sin6_port: a.port().to_be(),
                    sin6_addr: in6_addr,
                    sin6_flowinfo: a.flowinfo(),
                    sin6_scope_id: a.scope_id(),
                };
                (
                    &sockaddr as *const sockaddr_in6 as *const sockaddr,
                    std::mem::size_of_val(&sockaddr) as socklen_t,
                )
            }
        };

        let result = unsafe { libc::connect(socket_fd, sock_addr, sock_addr_len) };

        if result < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(socket_fd) };
            return Err(CommonError::SocketConnectFailed(err));
        }

        Ok(TimestampedTcpSocket { inner: socket_fd })
    }
}

impl<'a> Socket<'a, TimestampedTcpSocket> for TimestampedTcpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> TimestampedTcpSocket {
        Self { inner: fd }
    }

    fn send(&self, message: impl BeBytes) -> Result<(usize, DateTime), CommonError> {
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
        Ok((result as usize, timestamp))
    }

    fn send_to(
        &self,
        _address: &SocketAddr,
        message: impl message_macro::BeBytes,
    ) -> Result<(usize, crate::time::DateTime), CommonError> {
        // Use the send method to send the data
        self.send(message)
    }

    fn receive(&self, buffer: &mut [u8]) -> Result<(usize, DateTime), CommonError> {
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
        Ok((result as usize, timestamp))
    }

    fn receive_from(
        &self,
        buffer: &mut [u8],
    ) -> Result<(usize, SocketAddr, DateTime), CommonError> {
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

        let peer_address = match addr_storage.ss_family as libc::c_int {
            AF_INET => {
                let sockaddr: *const sockaddr_in = &addr_storage as *const _ as *const sockaddr_in;
                let sockaddr: &sockaddr_in = unsafe { &*sockaddr };
                let ip = Ipv4Addr::from(sockaddr.sin_addr.s_addr.to_le_bytes());
                let port = u16::from_be(sockaddr.sin_port);
                SocketAddr::V4(std::net::SocketAddrV4::new(ip, port))
            }
            AF_INET6 => {
                let sockaddr: *const sockaddr_in6 =
                    &addr_storage as *const _ as *const sockaddr_in6;
                let sockaddr: &sockaddr_in6 = unsafe { &*sockaddr };
                let ip = Ipv6Addr::from(sockaddr.sin6_addr.s6_addr);
                let port = u16::from_be(sockaddr.sin6_port);
                let flowinfo = sockaddr.sin6_flowinfo;
                let scope_id = sockaddr.sin6_scope_id;
                SocketAddr::V6(std::net::SocketAddrV6::new(ip, port, flowinfo, scope_id))
            }
            _ => {
                return Err(CommonError::UnknownAddressFamily);
            }
        };

        Ok((result, peer_address, timestamp))
    }
}
