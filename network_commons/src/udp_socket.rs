use bebytes::BeBytes;
use libc::{iovec, mmsghdr, msghdr, recvmsg, sendmsg, sockaddr_storage, timespec};

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
use crate::socket::{
    init_vec_of_mmsghdr, retrieve_data_from_header, storage_to_socket_addr, to_sockaddr, Socket,
};
use crate::time::DateTime;

/// The maximum number of messages that can be received at once.
const MAX_MSG: usize = 2;
const CMSG_SPACE_SIZE: usize = 128;

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
        #[cfg(target_os = "linux")]
        let socket_fd = match addr {
            SocketAddr::V4(_) => unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) },
            SocketAddr::V6(_) => unsafe { libc::socket(libc::AF_INET6, libc::SOCK_DGRAM, 0) },
        };

        if socket_fd < 0 {
            return Err(CommonError::SocketCreateFailed(io::Error::last_os_error()));
        }

        let (sock_addr, sock_addr_len) = to_sockaddr(addr);
        let sock_addr_ptr = &sock_addr as *const _ as *const libc::sockaddr;
        if unsafe { libc::bind(socket_fd, sock_addr_ptr, sock_addr_len) } < 0 {
            return Err(CommonError::SocketBindFailed(io::Error::last_os_error()));
        }

        Ok(Self { inner: socket_fd })
    }

    /// In a traditional UDP socket implementation the connect method
    /// sets the default destination address for future sends and limits
    /// incoming packets to come only from the specified address.
    pub fn connect(&self, address: SocketAddr) -> Result<i32, CommonError> {
        let (addr, len) = to_sockaddr(&address);
        let res = libc_call!(connect(self.inner, &addr as *const _ as *const _, len))
            .map_err(CommonError::Io)?;

        Ok(res)
    }

    pub fn receive_from_multiple(
        &self,
        buffers: &mut [[u8; 4096]],
        num_messages: usize,
    ) -> Result<Vec<(usize, SocketAddr, DateTime)>, CommonError> {
        let fd = self.as_raw_fd();
        let mut addr_storage: Vec<sockaddr_storage> =
            vec![unsafe { std::mem::zeroed() }; num_messages];
        let mut msg_hdrs: Vec<mmsghdr> = Vec::new();
        let mut iovecs: Vec<iovec> = Vec::with_capacity(num_messages);
        for (i, buffer) in buffers.iter_mut().take(num_messages).enumerate() {
            iovecs.push(iovec {
                iov_base: buffer.as_mut_ptr() as *mut libc::c_void,
                iov_len: buffer.len(),
            });

            msg_hdrs.push(mmsghdr {
                msg_hdr: msghdr {
                    msg_name: &mut addr_storage[i] as *mut _ as *mut libc::c_void,
                    msg_namelen: std::mem::size_of_val(&addr_storage[i]) as u32,
                    msg_iov: &mut iovecs[i] as *mut iovec,
                    msg_iovlen: iovecs.len(),
                    msg_control: [0; CMSG_SPACE_SIZE].as_mut_ptr() as *mut libc::c_void,
                    msg_controllen: CMSG_SPACE_SIZE,
                    msg_flags: 0,
                },
                msg_len: std::mem::size_of::<msghdr>() as u32,
            });
        }

        let (mut timestamp, result) = match recvmmsg_timestamped(fd, &mut msg_hdrs, num_messages) {
            Ok(value) => value,
            Err(value) => return value,
        };

        let mut received_data = Vec::new();
        for i in 0..result as usize {
            // let socket_addr = addresses[i];
            let socket_addr = storage_to_socket_addr(&addr_storage[i])?;
            if let Ok(datetime) = retrieve_data_from_header(&msg_hdrs[i].msg_hdr) {
                timestamp = datetime;
            };
            received_data.push((msg_hdrs[i].msg_len as usize, socket_addr, timestamp));
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
    pub fn retrieve_tx_timestamps(
        &mut self,
        addresses: &mut [SocketAddr],
    ) -> Result<Vec<DateTime>, CommonError> {
        let mut timestamps = Vec::new();
        // log::info!("Addresses {:?}", addresses);
        let mut msg_buffers: [[u8; 4096]; MAX_MSG] = unsafe { core::mem::zeroed() };
        let mut msgvec = init_vec_of_mmsghdr(MAX_MSG, &mut msg_buffers, addresses);

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
                if let Ok(date_time) = retrieve_data_from_header(&msg.msg_hdr) {
                    timestamps.push(date_time);
                }
            }
            Ok(timestamps)
        } else {
            let last_os_error = std::io::Error::last_os_error();
            Err(CommonError::Io(last_os_error))
        }
    }

    /// Attempts to receive a single timestamped error message from the socket.
    ///
    /// Returns a tuple containing the size of the received message,
    /// the sender's address, and the timestamp of the message.
    pub fn receive_error(&mut self) -> Result<(usize, SocketAddr, DateTime), CommonError> {
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

            let socket_addr = storage_to_socket_addr(&addr_storage)?;
            if res >= 0 {
                let datetime = retrieve_data_from_header(&msgh)?;
                return Ok((res as usize, socket_addr, datetime));
            } else {
                let err = std::io::Error::last_os_error();
                return Err(CommonError::Io(err));
            }
        }
    }
}

fn recvmmsg_timestamped(
    fd: i32,
    msg_hdrs: &mut Vec<mmsghdr>,
    num_messages: usize,
) -> Result<(DateTime, i32), Result<Vec<(usize, SocketAddr, DateTime)>, CommonError>> {
    let mut timeout = timespec {
        tv_sec: 0,
        tv_nsec: 100000, // 100us
    };
    let timestamp = DateTime::utc_now();
    let result = unsafe {
        libc::recvmmsg(
            fd,
            msg_hdrs.as_mut_ptr(),
            num_messages as u32,
            0,
            &mut timeout as *mut timespec, // wait for 1ms
        )
    };
    if result < 0 {
        let last_os_error = io::Error::last_os_error();
        return Err(Err(last_os_error.into()));
    }
    Ok((timestamp, result))
}

/// Implementation of the `Socket` trait for `TimestampedUdpSocket`.
impl Socket<TimestampedUdpSocket> for TimestampedUdpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> TimestampedUdpSocket {
        Self { inner: fd }
    }

    fn send(&self, buffer: impl BeBytes) -> Result<(isize, DateTime), CommonError> {
        let data = buffer.to_be_bytes();
        let length = data.len();

        let timestamp = DateTime::utc_now();
        let result = libc_call!(send(self.inner, data.as_ptr() as *const _, length, 0))
            .map_err(CommonError::Io)?;

        Ok((result, timestamp))
    }

    fn send_to(
        &self,
        address: &SocketAddr,
        message: impl BeBytes,
    ) -> Result<(isize, DateTime), CommonError> {
        let fd = self.as_raw_fd();
        let utc_now: DateTime;
        let bytes = message.to_be_bytes();
        let iov = [IoSlice::new(&bytes)];

        let (mut sock_addr, _len) = to_sockaddr(address);
        log::trace!("Sending to {:?}", sock_addr.sa_data);
        let msg = msghdr {
            msg_name: &mut sock_addr as *mut _ as *mut libc::c_void,
            msg_namelen: core::mem::size_of_val(&sock_addr) as u32,
            msg_iov: iov.as_ptr() as *mut libc::iovec,
            msg_iovlen: iov.len(),
            msg_control: std::ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: 0,
        };
        utc_now = DateTime::utc_now();
        let result = unsafe { sendmsg(fd, &msg, 0) };
        Ok((result, utc_now))
    }

    fn receive(&self, _buffer: &mut [u8]) -> Result<(isize, DateTime), CommonError> {
        unimplemented!()
    }

    fn receive_from(
        &self,
        buffer: &mut [u8],
    ) -> Result<(isize, SocketAddr, DateTime), CommonError> {
        let fd = self.as_raw_fd();
        let mut addr_storage: sockaddr_storage = unsafe { core::mem::zeroed() };

        let iov = [IoSliceMut::new(buffer)];
        let mut msg: msghdr = unsafe { core::mem::zeroed() };
        msg.msg_name = &mut addr_storage as *mut _ as *mut libc::c_void;
        msg.msg_namelen = core::mem::size_of_val(&addr_storage) as u32;
        msg.msg_iov = iov.as_ptr() as *mut iovec;
        msg.msg_iovlen = iov.len();
        const SPACE_SIZE: usize =
            unsafe { libc::CMSG_SPACE(core::mem::size_of::<libc::timeval>() as u32) as usize + 8 };
        let mut cmsg_space: [u8; SPACE_SIZE] = unsafe { core::mem::zeroed() };
        msg.msg_control = cmsg_space.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = cmsg_space.len();

        // Getting the backup timestamp right before the recvmsg call
        let mut timestamp = DateTime::utc_now();

        let n = unsafe { recvmsg(fd, &mut msg, 0) };
        if n < 0 {
            return Err(CommonError::Io(std::io::Error::last_os_error()));
        }

        // let socket_addr = match addr_storage.ss_family as i32 {
        //     libc::AF_INET => {
        //         let sockaddr: &libc::sockaddr_in = unsafe { core::mem::transmute(&addr_storage) };
        //         SocketAddr::new(
        //             IpAddr::V4(Ipv4Addr::from(sockaddr.sin_addr.s_addr.to_be())),
        //             sockaddr.sin_port.to_be(),
        //         )
        //     }
        //     libc::AF_INET6 => {
        //         let sockaddr: &libc::sockaddr_in6 = unsafe { core::mem::transmute(&addr_storage) };
        //         SocketAddr::new(
        //             IpAddr::V6(Ipv6Addr::from(sockaddr.sin6_addr.s6_addr)),
        //             sockaddr.sin6_port.to_be(),
        //         )
        //     }
        //     _ => return Err(CommonError::UnknownAddressFamily),
        // };
        let socket_addr = storage_to_socket_addr(unsafe { core::mem::transmute(msg.msg_name) })?;
        log::warn!("Socket address: {:?}", socket_addr);
        if let Ok(date_time) = retrieve_data_from_header(&msg) {
            timestamp = date_time;
            log::info!("Timestamp: {:?}", timestamp);
        };

        Ok((n, socket_addr, timestamp))
    }
}

#[derive(BeBytes, PartialEq, Debug, Clone)]
struct Message {
    pub data: Vec<u8>,
}
