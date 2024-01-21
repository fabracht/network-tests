use bebytes::BeBytes;
use libc::{iovec, mmsghdr, msghdr, recvmsg, sendmsg, sockaddr_storage, timespec};

use std::io::{self, IoSliceMut};

use std::os::fd::{AsRawFd, RawFd};
use std::ptr;
use std::{io::IoSlice, net::SocketAddr, ops::Deref};

use crate::error::CommonError;
use crate::libc_call;
use crate::socket::{
    init_vec_of_mmsghdr, retrieve_data_from_header, socketaddr_to_sockaddr, storage_to_socket_addr,
    to_msghdr, Socket, DEFAULT_BUFFER_SIZE,
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

        let (sock_addr, sock_addr_len) = socketaddr_to_sockaddr(addr);
        let sock_addr_ptr = &sock_addr as *const _;
        if unsafe { libc::bind(socket_fd, sock_addr_ptr, sock_addr_len) } < 0 {
            return Err(CommonError::SocketBindFailed(io::Error::last_os_error()));
        }

        Ok(Self { inner: socket_fd })
    }

    /// In a traditional UDP socket implementation the connect method
    /// sets the default destination address for future sends and limits
    /// incoming packets to come only from the specified address.
    pub fn connect(&self, address: SocketAddr) -> Result<i32, CommonError> {
        let (addr, len) = socketaddr_to_sockaddr(&address);
        let res = libc_call!(connect(self.inner, &addr as *const _ as *const _, len))
            .map_err(CommonError::Io)?;

        Ok(res)
    }

    pub fn receive_from_multiple(
        &self,
        buffers: &mut [[u8; DEFAULT_BUFFER_SIZE]],
        num_messages: usize,
    ) -> Result<Vec<(usize, SocketAddr, DateTime)>, CommonError> {
        let fd = self.as_raw_fd();
        let mut msg_hdrs: Vec<mmsghdr> = Vec::new();
        for buffer in buffers.iter_mut() {
            let mut addr_storage: SocketAddr = unsafe { std::mem::zeroed() };
            let buffer_ptr = buffer.as_mut_ptr();
            let msg_iov = iovec {
                iov_base: buffer_ptr as *mut libc::c_void,
                iov_len: buffer.len(),
            };
            let msg_hdr = msghdr {
                msg_name: &mut addr_storage as *mut _ as *mut libc::c_void,
                msg_namelen: std::mem::size_of_val(&addr_storage) as u32,
                msg_iov: &msg_iov as *const _ as *mut _,
                msg_iovlen: core::mem::size_of_val(&msg_iov),
                msg_control: [0; CMSG_SPACE_SIZE].as_mut_ptr() as *mut libc::c_void,
                msg_controllen: CMSG_SPACE_SIZE,
                msg_flags: 0,
            };
            msg_hdrs.push(mmsghdr {
                msg_hdr,
                msg_len: std::mem::size_of::<msghdr>() as u32,
            });
        }
        log::trace!("iov ptr {:?}", msg_hdrs[0].msg_hdr.msg_iov);

        let (mut timestamp, result) = match recvmmsg_timestamped(fd, &mut msg_hdrs, num_messages) {
            Ok(value) => value,
            Err(e) => {
                log::debug!("Error receiving multiple messages: {:?}", e);
                return Err(e);
            }
        };
        log::trace!("iov ptr {:?}", msg_hdrs[0].msg_hdr.msg_iov);

        let mut received_data = Vec::new();
        for mmsg_hdr in msg_hdrs.iter().take(result as usize) {
            let socket_addr = storage_to_socket_addr(unsafe {
                &*(mmsg_hdr.msg_hdr.msg_name as *const libc::sockaddr_storage)
            })?;
            if let Ok(datetime) = retrieve_data_from_header(&mmsg_hdr.msg_hdr) {
                timestamp = datetime;
                log::debug!("Timestamp {:?} from {:?}", timestamp, socket_addr);
            };
            received_data.push((mmsg_hdr.msg_len as usize, socket_addr, timestamp));
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
        let mut msg_buffers: [[u8; DEFAULT_BUFFER_SIZE]; MAX_MSG] = unsafe { core::mem::zeroed() };
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
    pub fn retrieve_tx_timestamp(&mut self) -> Result<(usize, SocketAddr, DateTime), CommonError> {
        let mut msg_buffer = [0u8; DEFAULT_BUFFER_SIZE];
        let mut address: SocketAddr = unsafe { core::mem::zeroed() };

        let mut msgh = to_msghdr(&mut msg_buffer, &mut address);

        #[cfg(target_os = "linux")]
        {
            let res = unsafe { libc::recvmsg(self.as_raw_fd(), &mut msgh, libc::MSG_ERRQUEUE) };
            let socket_addr = storage_to_socket_addr(unsafe {
                &*(msgh.msg_name as *const libc::sockaddr_storage)
            })?;
            if res >= 0 {
                let datetime = retrieve_data_from_header(&msgh)?;
                Ok((res as usize, socket_addr, datetime))
            } else {
                let err = std::io::Error::last_os_error();
                Err(CommonError::Io(err))
            }
        }
    }
}

fn recvmmsg_timestamped(
    fd: i32,
    msg_hdrs: &mut [mmsghdr],
    num_messages: usize,
) -> Result<(DateTime, i32), CommonError> {
    let timestamp = DateTime::utc_now();
    let result = unsafe {
        libc::recvmmsg(
            fd,
            msg_hdrs.as_mut_ptr(),
            num_messages as u32,
            0,
            ptr::null::<timespec>() as *mut _,
        )
    };
    log::trace!("First buffer {:?}", unsafe {
        msg_hdrs[0].msg_hdr.msg_iov.read().iov_len
    });
    if result < 0 {
        let last_os_error = io::Error::last_os_error();
        log::debug!("Error receiving multiple messages: {:?}", last_os_error);
        return Err(last_os_error.into());
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
        let bytes = message.to_be_bytes();
        let iov = [IoSlice::new(&bytes)];

        let (mut sock_addr, _len) = socketaddr_to_sockaddr(address);
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
        let utc_now = DateTime::utc_now();
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
        const SPACE_SIZE: usize = unsafe {
            libc::CMSG_SPACE(core::mem::size_of::<libc::timeval>() as u32) as usize * MAX_MSG
        };
        let mut cmsg_space: [u8; SPACE_SIZE] = unsafe { core::mem::zeroed() };
        msg.msg_control = cmsg_space.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = cmsg_space.len();

        // Getting the backup timestamp right before the recvmsg call
        let mut timestamp = DateTime::utc_now();

        let n = unsafe { recvmsg(fd, &mut msg, 0) };
        if n < 0 {
            return Err(CommonError::Io(std::io::Error::last_os_error()));
        }

        let socket_addr =
            storage_to_socket_addr(unsafe { &*(msg.msg_name as *const libc::sockaddr_storage) })?;
        log::debug!("Socket address: {:?}", socket_addr);
        if let Ok(date_time) = retrieve_data_from_header(&msg) {
            timestamp = date_time;
            log::debug!("Timestamp: {:?}", timestamp);
        };

        Ok((n, socket_addr, timestamp))
    }
}

#[derive(BeBytes, PartialEq, Debug, Clone)]
struct Message {
    pub data: Vec<u8>,
}
