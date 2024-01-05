use super::error::CommonError;
use crate::libc_call;
use crate::time::DateTime;
use bebytes::BeBytes;
use std::io::IoSlice;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsRawFd, RawFd};

const CMSG_SPACE_SIZE: usize = 128;

/// A trait representing a socket that can send and receive data.
pub trait Socket<T: AsRawFd>: Sized + AsRawFd {
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
    ///
    /// # Errors
    ///
    /// * `CommonError::Io` - An I/O error occurred.
    ///
    /// # Warning
    ///
    /// This function will error out when called on a non-connected socket. Always ensure that
    /// the socket is connected before attempting to send data.
    fn send(&self, message: impl BeBytes) -> Result<(isize, DateTime), CommonError>;

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
    ) -> Result<(isize, DateTime), CommonError>;

    /// Receives data from the socket into the given buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to receive the data into.
    ///
    /// # Returns
    ///
    /// A `Result` that contains the number of bytes received and the DateTime when the message was received, or a `CommonError` if an error occurred.
    fn receive(&self, buffer: &mut [u8]) -> Result<(isize, DateTime), CommonError>;

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
        -> Result<(isize, SocketAddr, DateTime), CommonError>;

    fn set_socket_options(
        &mut self,
        level: i32,
        name: i32,
        value: Option<i32>,
    ) -> Result<i32, CommonError> {
        libc_call!(setsockopt(
            self.as_raw_fd(),
            level,
            name,
            &value.unwrap_or(0) as *const std::ffi::c_int as *const std::ffi::c_void,
            std::mem::size_of_val(&value) as libc::socklen_t
        ))
        .map_err(CommonError::Io)
    }

    fn set_fcntl_options(&self) -> Result<i32, CommonError> {
        // Get current flags
        let flags = libc_call!(fcntl(self.as_raw_fd(), libc::F_GETFL)).map_err(CommonError::Io)?;

        // Add O_NONBLOCK and O_CLOEXEC to the flags
        let new_flags = flags | libc::O_NONBLOCK | libc::O_CLOEXEC;

        // Set the new flags
        libc_call!(fcntl(self.as_raw_fd(), libc::F_SETFL, new_flags)).map_err(CommonError::Io)
    }

    fn set_timestamping_options(&mut self) -> Result<i32, CommonError> {
        let value = libc::SOF_TIMESTAMPING_SOFTWARE
            | libc::SOF_TIMESTAMPING_RX_SOFTWARE
            | libc::SOF_TIMESTAMPING_TX_SOFTWARE;
        self.set_socket_options(libc::SOL_SOCKET, libc::SO_TIMESTAMPING, Some(value as i32))
    }
}

pub fn to_sockaddr(addr: &SocketAddr) -> (libc::sockaddr, u32) {
    let mut storage: libc::sockaddr_storage = unsafe { core::mem::zeroed() };
    log::debug!("addr: {}", addr.to_string());
    let (sock_addr, sock_addr_len) = match addr {
        SocketAddr::V4(a) => {
            let sockaddr_in: *mut libc::sockaddr_in =
                &mut storage as *mut _ as *mut libc::sockaddr_in;
            unsafe {
                (*sockaddr_in).sin_family = libc::AF_INET as libc::sa_family_t;
                (*sockaddr_in).sin_port = a.port().to_be();
                (*sockaddr_in).sin_addr.s_addr = u32::from_ne_bytes(a.ip().octets());
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
    (unsafe { sock_addr.read() }, sock_addr_len)
}

pub fn socket_addr_to_storage(addr: &SocketAddr) -> Result<libc::sockaddr_storage, String> {
    let mut storage: libc::sockaddr_storage = unsafe { core::mem::zeroed() };
    match addr {
        SocketAddr::V4(addr_v4) => {
            let sockaddr_in: *mut libc::sockaddr_in =
                &mut storage as *mut _ as *mut libc::sockaddr_in;
            unsafe {
                (*sockaddr_in).sin_family = libc::AF_INET as libc::sa_family_t;
                (*sockaddr_in).sin_port = addr_v4.port().to_be();
                (*sockaddr_in).sin_addr.s_addr = u32::from_ne_bytes(addr_v4.ip().octets());
            }
        }
        SocketAddr::V6(addr_v6) => {
            let sockaddr_in6: *mut libc::sockaddr_in6 =
                &mut storage as *mut _ as *mut libc::sockaddr_in6;
            unsafe {
                (*sockaddr_in6).sin6_family = libc::AF_INET6 as libc::sa_family_t;
                (*sockaddr_in6).sin6_port = addr_v6.port().to_be();
                (*sockaddr_in6).sin6_addr.s6_addr = addr_v6.ip().octets();
                (*sockaddr_in6).sin6_flowinfo = addr_v6.flowinfo();
                (*sockaddr_in6).sin6_scope_id = addr_v6.scope_id();
            }
        }
    };
    Ok(storage)
}

pub fn to_msghdr(bytes: &[u8], address: &SocketAddr) -> libc::msghdr {
    let iov = [IoSlice::new(bytes)];
    let (mut sockaddr, _) = to_sockaddr(address);

    let msg = libc::msghdr {
        msg_name: &mut sockaddr as *mut _ as *mut libc::c_void,
        msg_namelen: core::mem::size_of_val(&sockaddr) as u32,
        msg_iov: iov.as_ptr() as *mut libc::iovec,
        msg_iovlen: iov.len(),
        msg_control: [0; CMSG_SPACE_SIZE].as_mut_ptr() as *mut libc::c_void,
        msg_controllen: CMSG_SPACE_SIZE,
        msg_flags: 0,
    };
    msg
}

pub fn retrieve_data_from_headers(
    msg_hdrs: Vec<libc::mmsghdr>,
) -> Result<Vec<DateTime>, CommonError> {
    let mut received_data = Vec::new();
    for msg_hdr in msg_hdrs.iter() {
        log::trace!("msg_hdr: {:?}", msg_hdr.msg_hdr.msg_name);
        let timestamp = retrieve_data_from_header(&msg_hdr.msg_hdr)?;
        received_data.push(timestamp);
    }
    Ok(received_data)
}

pub fn retrieve_data_from_header(msg_hdr: &libc::msghdr) -> Result<DateTime, CommonError> {
    let mut cmsg_ptr = unsafe { libc::CMSG_FIRSTHDR(core::mem::transmute(msg_hdr)) };

    while !cmsg_ptr.is_null() {
        unsafe {
            // let cmsg = unsafe { &*(cmsg_ptr as *const cmsghdr) };
            if (*cmsg_ptr).cmsg_level == libc::SOL_SOCKET
                && (*cmsg_ptr).cmsg_type == libc::SCM_TIMESTAMPING
            {
                let ts_ptr = libc::CMSG_DATA(cmsg_ptr) as *const [libc::timespec; 3];
                let ts = { *ts_ptr }[0]; // Index 0 for software timestamps
                return Ok(DateTime::from_timespec(ts));
            }
            // Check for TOS value
            if (*cmsg_ptr).cmsg_level == libc::IPPROTO_IP && (*cmsg_ptr).cmsg_type == libc::IP_TOS {
                let tos_value: u8 = *(libc::CMSG_DATA(cmsg_ptr) as *const u8);
                log::info!("TOS value: {}", tos_value);
            }
            cmsg_ptr = libc::CMSG_NXTHDR(core::mem::transmute(msg_hdr), cmsg_ptr);
        }
    }
    Err(CommonError::Generic("No tx timestamp found".to_string()))
}

pub fn storage_to_socket_addr(
    addr_storage: &libc::sockaddr_storage,
) -> Result<SocketAddr, CommonError> {
    let socket_addr = match addr_storage.ss_family as i32 {
        libc::AF_INET => {
            let sockaddr: &libc::sockaddr_in = unsafe { core::mem::transmute(addr_storage) };
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
    Ok(socket_addr)
}

pub fn init_vec_of_mmsghdr(
    max_msg: usize,
    msg_buffers: &mut [[u8; 4096]],
    addresses: &mut [SocketAddr],
) -> Vec<libc::mmsghdr> {
    let mut msgvec: Vec<libc::mmsghdr> = vec![unsafe { core::mem::zeroed() }; max_msg];
    for (i, (msg, buffer)) in msgvec
        .iter_mut()
        .zip(&mut msg_buffers.iter_mut())
        .enumerate()
    {
        let socket_addr_index = i % addresses.len();
        msg.msg_hdr = to_msghdr(buffer, &mut addresses[socket_addr_index]);
    }
    msgvec
}
