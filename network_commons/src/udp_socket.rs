use bebytes::BeBytes;
use libc::{
    in6_addr, iovec, mmsghdr, msghdr, recvfrom, recvmmsg, sendmsg, sockaddr_in, sockaddr_in6,
    sockaddr_storage, timespec,
};

use std::io::{self, IoSliceMut};
use std::net::Ipv6Addr;
use std::os::fd::{AsRawFd, RawFd};
use std::{
    io::IoSlice,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
};

use crate::error::CommonError;
use crate::libc_call;
use crate::socket::Socket;
use crate::time::{DateTime, ScmTimestamping};

/// `TimestampedUdpSocket` is a wrapper around a raw file descriptor for a socket.
/// It provides methods for sending and receiving data over UDP, with timestamping capabilities.
pub struct TimestampedUdpSocket {
    inner: RawFd,
}

/// When a `TimestampedUdpSocket` goes out of scope, we want to ensure it is properly closed.
/// The `Drop` trait is implemented to automatically close the socket when it is dropped.
impl Drop for TimestampedUdpSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.inner) };
    }
}

/// The `AsRawFd` trait is implemented to allow us to access the raw file descriptor of the socket.
impl AsRawFd for TimestampedUdpSocket {
    /// Returns the raw file descriptor of the socket.
    fn as_raw_fd(&self) -> RawFd {
        self.inner
    }
}

/// Allows conversion from a mutable reference to an i32 to a `TimestampedUdpSocket`.
impl From<&mut i32> for TimestampedUdpSocket {
    /// Creates a new `TimestampedUdpSocket` from a mutable reference to an i32.
    fn from(value: &mut i32) -> Self {
        Self::new(value.as_raw_fd())
    }
}

impl Deref for TimestampedUdpSocket {
    type Target = RawFd;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TimestampedUdpSocket {
    /// Constructs a new `TimestampedUdpSocket` from a given raw file descriptor.
    pub fn new(socket: RawFd) -> Self {
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
            SocketAddr::V4(_) => unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) },
            SocketAddr::V6(_) => unsafe { libc::socket(libc::AF_INET6, libc::SOCK_DGRAM, 0) },
        };

        if socket_fd < 0 {
            return Err(CommonError::SocketCreateFailed(io::Error::last_os_error()));
        }

        let mut storage: libc::sockaddr_storage = unsafe { core::mem::zeroed() };
        let (sock_addr, sock_addr_len) = match addr {
            SocketAddr::V4(a) => {
                let sockaddr_in: *mut libc::sockaddr_in =
                    &mut storage as *mut _ as *mut libc::sockaddr_in;
                unsafe {
                    (*sockaddr_in).sin_family = libc::AF_INET as libc::sa_family_t;
                    (*sockaddr_in).sin_port = a.port().to_be();
                    (*sockaddr_in).sin_addr.s_addr = u32::from_be_bytes(a.ip().octets());
                }
                (
                    sockaddr_in as *const libc::sockaddr,
                    core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                )
            }
            SocketAddr::V6(a) => {
                let sockaddr_in6: *mut libc::sockaddr_in6 =
                    &mut storage as *mut _ as *mut libc::sockaddr_in6;
                unsafe {
                    (*sockaddr_in6).sin6_family = libc::AF_INET6 as libc::sa_family_t;
                    (*sockaddr_in6).sin6_port = a.port().to_be();
                    (*sockaddr_in6)
                        .sin6_addr
                        .s6_addr
                        .copy_from_slice(&a.ip().octets());
                    (*sockaddr_in6).sin6_flowinfo = a.flowinfo();
                    (*sockaddr_in6).sin6_scope_id = a.scope_id();
                }
                (
                    sockaddr_in6 as *const libc::sockaddr,
                    core::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
                )
            }
        };

        if unsafe { libc::bind(socket_fd, sock_addr, sock_addr_len) } < 0 {
            return Err(CommonError::SocketBindFailed(io::Error::last_os_error()));
        }

        Ok(Self { inner: socket_fd })
    }

    /// In a traditional UDP socket implementation the connect method
    /// sets the default destination address for future sends and limits
    /// incoming packets to come only from the specified address.
    pub fn connect(&self, address: SocketAddr) -> Result<i32, CommonError> {
        let (ip, port) = match address {
            SocketAddr::V4(addr) => (*addr.ip(), addr.port()),
            SocketAddr::V6(_) => {
                return Err(CommonError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "IPv6 is not supported",
                )))
            }
        };

        let addr = sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port: port.to_be(),
            sin_addr: libc::in_addr {
                s_addr: u32::from(ip).to_be(),
            },
            sin_zero: [0; 8],
        };

        let res = libc_call!(connect(
            self.inner,
            &addr as *const _ as *const _,
            core::mem::size_of::<sockaddr_in>() as u32
        ))
        .map_err(CommonError::Io)?;

        Ok(res)
    }

    pub fn receive_from_multiple(
        &self,
        buffers: &mut [[u8; 1024]],
        num_messages: usize,
    ) -> Result<Vec<(usize, SocketAddr, DateTime)>, io::Error> {
        let fd = self.as_raw_fd();

        let mut msg_hdrs: Vec<mmsghdr> = Vec::with_capacity(num_messages);
        let mut timestamps: Vec<timespec> = Vec::with_capacity(num_messages);

        for buffer in buffers.iter_mut().take(num_messages) {
            let mut addr_storage: sockaddr_storage = unsafe { core::mem::zeroed() };
            let mut msg_hdr: msghdr = unsafe { core::mem::zeroed() };

            let iov = iovec {
                iov_base: buffer.as_mut_ptr() as *mut libc::c_void,
                iov_len: buffer.len(),
            };

            msg_hdr.msg_name = &mut addr_storage as *mut _ as *mut libc::c_void;
            msg_hdr.msg_namelen = core::mem::size_of_val(&addr_storage) as u32;
            msg_hdr.msg_iov = &iov as *const iovec as *mut iovec;
            msg_hdr.msg_iovlen = 1;

            msg_hdrs.push(mmsghdr {
                msg_hdr,
                msg_len: 0,
            });

            timestamps.push(unsafe { core::mem::zeroed() });
        }

        let result = unsafe {
            recvmmsg(
                fd,
                msg_hdrs.as_mut_ptr(),
                num_messages as u32,
                0,
                timestamps.as_mut_ptr(),
            )
        };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut received_data = Vec::new();

        for i in 0..result as usize {
            let msg_hdr = &msg_hdrs[i].msg_hdr;
            let addr_storage: sockaddr_storage =
                unsafe { *(msg_hdr.msg_name as *mut sockaddr_storage) };
            let socket_addr = match addr_storage.ss_family as i32 {
                libc::AF_INET => {
                    let sockaddr: &libc::sockaddr_in =
                        unsafe { core::mem::transmute(&addr_storage) };
                    let ip_bytes = sockaddr.sin_addr.s_addr.to_be_bytes();
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(
                            ip_bytes[3],
                            ip_bytes[2],
                            ip_bytes[1],
                            ip_bytes[0],
                        )),
                        sockaddr.sin_port.to_be(),
                    )
                }
                libc::AF_INET6 => {
                    let sockaddr: &libc::sockaddr_in6 =
                        unsafe { core::mem::transmute(&addr_storage) };
                    SocketAddr::new(
                        IpAddr::V6(Ipv6Addr::from(sockaddr.sin6_addr.s6_addr)),
                        sockaddr.sin6_port.to_be(),
                    )
                }
                _ => continue, // Skip on unknown address family
            };

            received_data.push((
                msg_hdrs[i].msg_len as usize,
                socket_addr,
                DateTime::from_timespec(timestamps[i]),
            ));
        }

        Ok(received_data)
    }

    /// Attempts to receive multiple timestamped error messages from the socket.
    ///
    /// Returns a vector of tuples, each containing the size of the received message,
    /// the sender's address, and the timestamp of the message.
    /// Attempts to receive multiple timestamped error messages from the socket.
    ///
    /// Returns a vector of tuples, each containing the size of the received message,
    /// the sender's address, and the timestamp of the message.
    pub fn receive_errors(&mut self) -> Result<Vec<(usize, SocketAddr, DateTime)>, CommonError> {
        const MAX_MSG: usize = 10;
        let mut timestamps: Vec<(usize, SocketAddr, DateTime)> = Vec::new();
        let mut msgvec: [libc::mmsghdr; MAX_MSG] = unsafe { core::mem::zeroed() };
        let mut msg_buffers: [[u8; 4096]; MAX_MSG] = unsafe { core::mem::zeroed() };

        for (msg, buffer) in msgvec.iter_mut().zip(&mut msg_buffers) {
            let mut iov = iovec {
                iov_base: buffer.as_mut_ptr() as *mut libc::c_void,
                iov_len: buffer.len(),
            };
            let mut sockaddr = sockaddr_in {
                sin_family: libc::AF_INET as u16,
                sin_port: 0u16.to_be(),
                sin_addr: libc::in_addr {
                    s_addr: 0u32.to_be(),
                },
                sin_zero: [0; 8],
            };
            msg.msg_hdr = msghdr {
                msg_name: &mut sockaddr as *mut _ as *mut libc::c_void,
                msg_namelen: core::mem::size_of_val(&sockaddr) as u32,
                msg_iov: &mut iov as *mut iovec,
                msg_iovlen: 1,
                msg_control: buffer.as_mut_ptr() as *mut libc::c_void,
                msg_controllen: buffer.len(),
                msg_flags: 0,
            };
        }

        let res = unsafe {
            libc::recvmmsg(
                self.as_raw_fd(),
                msgvec.as_mut_ptr(),
                msgvec.len() as u32,
                libc::MSG_ERRQUEUE,
                std::ptr::null_mut::<timespec>(),
            )
        };

        if res >= 0 {
            for msg in &msgvec {
                let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg.msg_hdr) };
                while !cmsg.is_null() {
                    unsafe {
                        if (*cmsg).cmsg_level == libc::SOL_SOCKET
                            && (*cmsg).cmsg_type == libc::SCM_TIMESTAMPING
                        {
                            let data = libc::CMSG_DATA(cmsg);
                            let ts = (data as *const ScmTimestamping).as_ref().unwrap();
                            let timestamp = DateTime::from_timespec(ts.ts_realtime);

                            let sockaddr = &mut *(msg.msg_hdr.msg_name as *mut sockaddr_in);
                            let ip_bytes = sockaddr.sin_addr.s_addr.to_be_bytes();
                            let socket_addr = SocketAddr::new(
                                IpAddr::V4(Ipv4Addr::new(
                                    ip_bytes[3],
                                    ip_bytes[2],
                                    ip_bytes[1],
                                    ip_bytes[0],
                                )),
                                sockaddr.sin_port.to_be(),
                            );

                            timestamps.push((msg.msg_len as usize, socket_addr, timestamp));
                        }
                        cmsg = libc::CMSG_NXTHDR(&msg.msg_hdr, cmsg);
                    }
                }
            }
            Ok(timestamps)
        } else {
            let error = format!("Failed to get error messages: {}", res);
            Err(CommonError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                error,
            )))
        }
    }

    /// Attempts to receive a single timestamped error message from the socket.
    ///
    /// Returns a tuple containing the size of the received message,
    /// the sender's address, and the timestamp of the message.
    pub fn receive_error(&mut self) -> Result<(usize, SocketAddr, DateTime), CommonError> {
        let mut timestamp = DateTime::utc_now();
        let mut iov_buffer = [0u8; 4096];
        let mut msg_buffer = [0u8; 4096];
        let mut addr_storage: sockaddr_storage = unsafe { core::mem::zeroed() };

        let mut iov = iovec {
            iov_base: iov_buffer.as_mut_ptr() as *mut libc::c_void,
            iov_len: iov_buffer.len(),
        };

        #[cfg(target_os = "linux")]
        let mut msgh = msghdr {
            msg_name: &mut addr_storage as *mut _ as *mut libc::c_void,
            msg_namelen: core::mem::size_of_val(&addr_storage) as u32,
            msg_iov: &mut iov as *mut iovec,
            msg_iovlen: 1,
            msg_control: msg_buffer.as_mut_ptr() as *mut libc::c_void,
            msg_controllen: msg_buffer.len(),
            msg_flags: 0,
        };

        #[cfg(target_os = "linux")]
        {
            let res = unsafe { libc::recvmsg(self.as_raw_fd(), &mut msgh, libc::MSG_ERRQUEUE) };

            if res >= 0 {
                let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msgh) };
                while !cmsg.is_null() {
                    unsafe {
                        log::debug!("cmsg_level: {}", (*cmsg).cmsg_level);
                        log::debug!("cmsg_type: {}", (*cmsg).cmsg_type);
                        let data = libc::CMSG_DATA(cmsg);

                        if (*cmsg).cmsg_level == libc::SOL_SOCKET
                            && (*cmsg).cmsg_type == libc::SCM_TIMESTAMPING
                        {
                            let ts = (data as *const ScmTimestamping).as_ref().unwrap();
                            timestamp = DateTime::from_timespec(ts.ts_realtime);
                        }
                    }
                    cmsg = unsafe { libc::CMSG_NXTHDR(&msgh, cmsg) };
                }
            } else {
                let err = std::io::Error::last_os_error();
                log::error!("recvmmsg failed: {}", err);
                return Err(CommonError::Io(err));
            }

            // let ip_bytes = sockaddr.sin_addr.s_addr.to_be_bytes();
            let socket_addr = match addr_storage.ss_family as i32 {
                libc::AF_INET => {
                    let sockaddr: &libc::sockaddr_in =
                        unsafe { core::mem::transmute(&addr_storage) };
                    let ip_bytes = sockaddr.sin_addr.s_addr.to_be_bytes();
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(
                            ip_bytes[3],
                            ip_bytes[2],
                            ip_bytes[1],
                            ip_bytes[0],
                        )),
                        sockaddr.sin_port.to_be(),
                    )
                }
                libc::AF_INET6 => {
                    let sockaddr: &libc::sockaddr_in6 =
                        unsafe { core::mem::transmute(&addr_storage) };
                    SocketAddr::new(
                        IpAddr::V6(Ipv6Addr::from(sockaddr.sin6_addr.s6_addr)),
                        sockaddr.sin6_port.to_be(),
                    )
                }
                _ => return Err(CommonError::UnknownAddressFamily),
            };
            Ok((res as usize, socket_addr, timestamp))
        }
    }
}

/// Implementation of the `Socket` trait for `TimestampedUdpSocket`.
impl<'a> Socket<'a, TimestampedUdpSocket> for TimestampedUdpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> TimestampedUdpSocket {
        Self { inner: fd }
    }

    fn send(&self, buffer: impl BeBytes) -> Result<(usize, DateTime), CommonError> {
        let data = buffer.to_be_bytes();
        let length = data.len();

        let timestamp = DateTime::utc_now();
        let result = libc_call!(send(self.inner, data.as_ptr() as *const _, length, 0))
            .map_err(CommonError::Io)?;

        Ok((result as usize, timestamp))
    }

    fn send_to(
        &self,
        address: &SocketAddr,
        message: impl BeBytes,
    ) -> Result<(usize, DateTime), CommonError> {
        let fd = self.as_raw_fd();
        let utc_now: DateTime;
        let bytes = message.to_be_bytes();

        let iov = [IoSlice::new(&bytes)];
        let result: isize;
        match address.ip() {
            IpAddr::V4(ipv4) => {
                log::debug!("ipv4 address {}", ipv4.to_string());

                #[cfg(target_os = "linux")]
                let mut sockaddr = sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: address.port().to_be(),
                    sin_addr: libc::in_addr {
                        s_addr: u32::from(ipv4).to_be(),
                    },
                    sin_zero: [0; 8],
                };

                #[cfg(target_os = "linux")]
                let msg = libc::msghdr {
                    msg_name: &mut sockaddr as *mut _ as *mut libc::c_void,
                    msg_namelen: core::mem::size_of_val(&sockaddr) as u32,
                    msg_iov: iov.as_ptr() as *mut libc::iovec,
                    msg_iovlen: iov.len(),
                    msg_control: std::ptr::null_mut(),
                    msg_controllen: 0,
                    msg_flags: 0,
                };
                utc_now = DateTime::utc_now();
                result = unsafe { sendmsg(fd, &msg, 0) };
            }
            IpAddr::V6(ipv6) => {
                log::debug!("ipv6 address {}", ipv6.to_string());

                #[cfg(target_os = "linux")]
                let mut sockaddr = sockaddr_in6 {
                    sin6_family: libc::AF_INET6 as u16,
                    sin6_port: address.port().to_be(),
                    sin6_addr: in6_addr {
                        s6_addr: ipv6.octets(),
                    },
                    sin6_flowinfo: 0,
                    sin6_scope_id: 0,
                };

                #[cfg(target_os = "linux")]
                let msg = libc::msghdr {
                    msg_name: &mut sockaddr as *mut _ as *mut libc::c_void,
                    msg_namelen: core::mem::size_of_val(&sockaddr) as u32,
                    msg_iov: iov.as_ptr() as *mut libc::iovec,
                    msg_iovlen: iov.len(),
                    msg_control: std::ptr::null_mut(),
                    msg_controllen: 0,
                    msg_flags: 0,
                };
                utc_now = DateTime::utc_now();
                result = unsafe { sendmsg(fd, &msg, 0) };
            }
        }

        Ok((result as usize, utc_now))
    }

    fn receive(&self, _buffer: &mut [u8]) -> Result<(usize, DateTime), CommonError> {
        unimplemented!()
    }
    fn receive_from(
        &self,
        buffer: &mut [u8],
    ) -> Result<(usize, SocketAddr, DateTime), CommonError> {
        let fd = self.as_raw_fd();
        let mut addr_storage: sockaddr_storage = unsafe { core::mem::zeroed() };

        let iov = [IoSliceMut::new(buffer)];
        let mut msg: msghdr = unsafe { core::mem::zeroed() };
        msg.msg_name = &mut addr_storage as *mut _ as *mut libc::c_void;
        msg.msg_namelen = core::mem::size_of_val(&addr_storage) as u32;
        msg.msg_iov = iov.as_ptr() as *mut iovec;
        msg.msg_iovlen = iov.len();
        let utc_now = DateTime::utc_now();

        let n = unsafe {
            recvfrom(
                fd,
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                0,
                &mut addr_storage as *const _ as *mut _,
                &mut core::mem::size_of_val(&addr_storage) as *const _ as *mut _,
            )
        };
        if n < 0 {
            return Err(CommonError::Io(std::io::Error::last_os_error()));
        }

        let socket_addr = match addr_storage.ss_family as i32 {
            libc::AF_INET => {
                let sockaddr: &libc::sockaddr_in = unsafe { core::mem::transmute(&addr_storage) };
                let ip_bytes = sockaddr.sin_addr.s_addr.to_be_bytes();
                SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(
                        ip_bytes[3],
                        ip_bytes[2],
                        ip_bytes[1],
                        ip_bytes[0],
                    )),
                    sockaddr.sin_port.to_be(),
                )
            }
            libc::AF_INET6 => {
                let sockaddr: &libc::sockaddr_in6 = unsafe { core::mem::transmute(&addr_storage) };
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::from(sockaddr.sin6_addr.s6_addr)),
                    sockaddr.sin6_port.to_be(),
                )
            }
            _ => return Err(CommonError::UnknownAddressFamily),
        };

        Ok((n as usize, socket_addr, utc_now))
    }
}
