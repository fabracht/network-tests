use libc::{iovec, msghdr, recvfrom, sa_family_t, sendmsg, sockaddr_in};
use std::{
    io::IoSlice,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
    os::fd::{AsRawFd, RawFd},
};

use crate::time::DateTime;

use super::{error::CommonError, message::Message};

pub trait Socket<'a, T: AsRawFd> {
    fn send(&self, messages: msghdr) -> Result<(usize, DateTime), CommonError>;
    fn send_to(
        &self,
        address: &SocketAddr,
        message: impl Message,
    ) -> Result<(usize, DateTime), CommonError>;
    fn receive(&self, buffer: &mut [u8]) -> Result<(usize, DateTime), CommonError>;
    fn receive_from(&self, buffer: &mut [u8])
        -> Result<(usize, SocketAddr, DateTime), CommonError>;
}

pub struct CustomUdpSocket {
    inner: RawFd,
}

impl Drop for CustomUdpSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.inner) };
    }
}

impl AsRawFd for CustomUdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.inner
    }
}

impl From<&mut i32> for CustomUdpSocket {
    fn from(value: &mut i32) -> Self {
        Self::new(value.as_raw_fd())
    }
}

impl Deref for CustomUdpSocket {
    type Target = RawFd;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl CustomUdpSocket {
    pub fn new(socket: RawFd) -> Self {
        Self { inner: socket }
    }

    pub fn set_socket_options(&mut self, name: i32, value: Option<i32>) -> Result<(), CommonError> {
        let _res = unsafe {
            libc::setsockopt(
                self.inner,
                libc::SOL_SOCKET,
                name,
                &value.unwrap_or(0) as *const std::ffi::c_int as *const std::ffi::c_void,
                std::mem::size_of::<std::ffi::c_int>() as u32,
            )
        };
        Ok(())
    }
}

impl<'a> Socket<'a, CustomUdpSocket> for CustomUdpSocket {
    fn send(&self, _buffer: msghdr) -> Result<(usize, DateTime), CommonError> {
        todo!()
    }

    fn send_to(
        &self,
        address: &SocketAddr,
        message: impl Message,
    ) -> Result<(usize, DateTime), CommonError> {
        let fd = self.as_raw_fd();
        let mut utc_now: DateTime;
        let bytes = message.to_bytes();
        let iov = [IoSlice::new(&bytes)];
        let result: isize;
        match address.ip() {
            IpAddr::V4(ipv4) => {
                log::info!("ipv4 address {}", ipv4.to_string());
                #[cfg(target_os = "macos")]
                let mut sockaddr = sockaddr_in {
                    sin_family: libc::AF_INET as u8,
                    sin_port: address.port().to_be(),
                    sin_addr: libc::in_addr {
                        s_addr: u32::from(ipv4).to_be(),
                    },
                    sin_zero: [0; 8],
                    sin_len: core::mem::size_of::<libc::sockaddr_in>() as u8,
                };

                #[cfg(target_os = "linux")]
                let mut sockaddr = sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: address.port().to_be(),
                    sin_addr: libc::in_addr {
                        s_addr: u32::from(ipv4).to_be(),
                    },
                    sin_zero: [0; 8],
                };

                #[cfg(target_os = "macos")]
                let msg = libc::msghdr {
                    msg_name: &mut sockaddr as *mut _ as *mut libc::c_void,
                    msg_namelen: std::mem::size_of_val(&sockaddr) as u32,
                    msg_iov: iov.as_ptr() as *mut libc::iovec,
                    msg_iovlen: iov.len() as i32,
                    msg_control: std::ptr::null_mut(),
                    msg_controllen: 0,
                    msg_flags: 0,
                };

                #[cfg(target_os = "linux")]
                let msg = libc::msghdr {
                    msg_name: &mut sockaddr as *mut _ as *mut libc::c_void,
                    msg_namelen: std::mem::size_of_val(&sockaddr) as u32,
                    msg_iov: iov.as_ptr() as *mut libc::iovec,
                    msg_iovlen: iov.len(),
                    msg_control: std::ptr::null_mut(),
                    msg_controllen: 0,
                    msg_flags: 0,
                };
                utc_now = DateTime::utc_now();
                result = unsafe { sendmsg(fd, &msg, 0) };
            }
            IpAddr::V6(_) => todo!(),
        }

        let mut msg_buffer = [0u8; 4096];
        let mut iov = iovec {
            iov_base: msg_buffer.as_mut_ptr() as *mut libc::c_void,
            iov_len: msg_buffer.len(),
        };

        #[cfg(target_os = "linux")]
        let mut msgh = msghdr {
            msg_name: std::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &mut iov as *mut iovec,
            msg_iovlen: 0,
            msg_control: msg_buffer.as_mut_ptr() as *mut libc::c_void,
            msg_controllen: msg_buffer.len(),
            msg_flags: 0,
        };
        #[cfg(target_os = "macos")]
        let msgh = msghdr {
            msg_name: std::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &mut iov as *mut iovec,
            msg_iovlen: 0,
            msg_control: msg_buffer.as_mut_ptr() as *mut libc::c_void,
            msg_controllen: msg_buffer.len() as u32,
            msg_flags: 0,
        };

        #[cfg(target_os = "linux")]
        {
            let res = unsafe { libc::recvmsg(fd, &mut msgh, libc::MSG_ERRQUEUE) };
            if res >= 0 {
                let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msgh) };
                while cmsg != std::ptr::null_mut() {
                    unsafe {
                        if (*cmsg).cmsg_level == libc::SOL_SOCKET
                            && (*cmsg).cmsg_type == libc::SCM_TIMESTAMPING
                        {
                            let ts = (libc::CMSG_DATA(cmsg) as *const ScmTimestamping)
                                .as_ref()
                                .unwrap();
                            // let sec = ts.ts_realtime.tv_sec;
                            // let nsec = ts.ts_realtime.tv_nsec as u32;
                            utc_now = DateTime::from_timespec(ts.ts_realtime);
                        }
                    }
                    cmsg = unsafe { libc::CMSG_NXTHDR(&msgh, cmsg) };
                }
            } else {
                log::error!("Failed to get error message: {}", res);
            }
            if result < 0 {
                log::error!("Error sending message: {}", std::io::Error::last_os_error());
            } else {
                log::debug!("Sent message");
            };
        }

        #[cfg(target_os = "macos")]
        {
            let mut buf = [0u8; 4096];
            let mut iov = libc::iovec {
                iov_base: buf.as_mut_ptr() as *mut libc::c_void,
                iov_len: buf.len(),
            };
            let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
            msg.msg_iov = &mut iov;
            msg.msg_iovlen = 1;
            let mut cmsg_buffer = [0u8; unsafe {
                libc::CMSG_SPACE(std::mem::size_of::<libc::timeval>() as u32) as usize
            }];
            msg.msg_control = cmsg_buffer.as_mut_ptr() as *mut libc::c_void;
            msg.msg_controllen = cmsg_buffer.len() as u32;
            let bytes_received = unsafe { libc::recvmsg(fd, &mut msg, 0) };
            if bytes_received < 0 {
                log::error!(
                    "Failed to get error message: {}",
                    std::io::Error::last_os_error()
                );
            } else if bytes_received > 0 {
                let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msgh) };
                while cmsg != std::ptr::null_mut() {
                    unsafe {
                        if (*cmsg).cmsg_level == libc::SOL_SOCKET
                            && (*cmsg).cmsg_type == libc::SCM_TIMESTAMP
                        {
                            let ts = (libc::CMSG_DATA(cmsg) as *const ScmTimestamping)
                                .as_ref()
                                .unwrap();
                            // let sec = ts.ts_realtime.tv_sec;
                            // let nsec = ts.ts_realtime.tv_nsec as u32;
                            utc_now = DateTime::from_timespec(ts.ts_realtime);
                        }
                    }
                    cmsg = unsafe { libc::CMSG_NXTHDR(&msgh, cmsg) };
                }
            }
        }

        Ok((result as usize, utc_now))
    }

    fn receive(&self, _buffer: &mut [u8]) -> Result<(usize, DateTime), CommonError> {
        todo!()
    }

    fn receive_from(
        &self,
        buffer: &mut [u8],
    ) -> Result<(usize, SocketAddr, DateTime), CommonError> {
        #[cfg(target_os = "linux")]
        let mut sockaddr = sockaddr_in {
            sin_family: libc::AF_INET as sa_family_t,
            sin_port: 0,
            sin_addr: libc::in_addr { s_addr: 0 },
            sin_zero: [0; 8],
        };
        #[cfg(target_os = "macos")]
        let mut sockaddr = sockaddr_in {
            sin_family: libc::AF_INET as sa_family_t,
            sin_port: 0,
            sin_addr: libc::in_addr { s_addr: 0 },
            sin_zero: [0; 8],
            sin_len: core::mem::size_of::<libc::sockaddr_in>() as u8,
        };

        let fd = self.as_raw_fd();
        // Receive the message using `recvfrom` from the libc crate
        let utc_now = DateTime::utc_now();
        let n = unsafe {
            recvfrom(
                fd,
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                0,
                &mut sockaddr as *const _ as *mut _,
                &mut std::mem::size_of_val(&sockaddr) as *const _ as *mut _,
            )
        };

        // Convert the message to a string
        let ip_bytes = sockaddr.sin_addr.s_addr.to_le_bytes();
        let socket_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(
                ip_bytes[0],
                ip_bytes[1],
                ip_bytes[2],
                ip_bytes[3],
            )),
            sockaddr.sin_port.to_be(),
        );

        Ok((n as usize, socket_addr, utc_now))
    }
}

pub fn _print_bytes(data: &[u8]) {
    for (i, byte) in data.iter().enumerate() {
        if i % 4 == 0 {
            if i > 0 {
                print!("    ");
                for j in i - 4..i {
                    if data.get(j).map(|&b| _is_printable(b)).unwrap_or(false) {
                        print!("{}", data[j] as char);
                    } else {
                        print!(".");
                    }
                }
                println!("");
            }
            print!("{:08x}: ", i);
        }
        print!("{:02x} ", byte);
    }

    let remainder = data.len() % 4;
    if remainder != 0 {
        for _ in remainder..4 {
            print!("   ");
        }
        print!("    ");
        let start = data.len() - remainder;
        for j in start..data.len() {
            if _is_printable(data[j]) {
                print!("{}", data[j] as char);
            } else {
                print!(".");
            }
        }
    }
    println!("");
}

fn _is_printable(byte: u8) -> bool {
    (0x20..=0x7E).contains(&byte)
}

#[derive(Debug)]
#[repr(C)]
pub struct ScmTimestamping {
    pub ts_realtime: libc::timespec,
    pub ts_mono: libc::timespec,
    pub ts_raw: libc::timespec,
}

#[cfg(target_os = "macos")]
pub fn set_timestamping_options(socket: &mut CustomUdpSocket) -> Result<(), CommonError> {
    let value = 1; // Enable the SO_TIMESTAMP option
    socket.set_socket_options(libc::SO_TIMESTAMP, Some(value))
}

#[cfg(target_os = "linux")]
pub fn set_timestamping_options(socket: &mut CustomUdpSocket) -> Result<(), CommonError> {
    let value = libc::SOF_TIMESTAMPING_SOFTWARE
        | libc::SOF_TIMESTAMPING_RX_SOFTWARE
        | libc::SOF_TIMESTAMPING_TX_SOFTWARE;
    socket.set_socket_options(libc::SO_TIMESTAMPING, Some(value as i32))
}
