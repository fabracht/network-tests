/// Module containing error handling components.
/// `CommonError` is an enum containing error variants which are likely to be used across different parts of the codebase.
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::sync::{PoisonError, RwLockWriteGuard, TryLockError};

/// A handy Result type specific to the common error set defined by `CommonError`.
pub type Result<T> = std::result::Result<T, CommonError>;

/// The set of all errors which can be produced in the system.
#[derive(Debug)]
pub enum CommonError {
    Io(std::io::Error),
    NotEnoughBytes(String),
    ConversionFromBytes(std::array::TryFromSliceError),
    AddrParseError(std::net::AddrParseError),
    Infallible(std::convert::Infallible),
    Lock,
    Dns(std::io::Error),
    Generic(String),
    ValidationError(validator::ValidationErrors),
    SendError(String),
    TryRecvError(String),
    IterError(String),
    SocketCreateFailed(std::io::Error),
    SocketConnectFailed(std::io::Error),
    SocketBindFailed(std::io::Error),
    SocketListenFailed(std::io::Error),
    SocketAcceptFailed(std::io::Error),
    SocketGetPeerName(std::io::Error),
    UnknownAddressFamily,
}

impl Display for CommonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            CommonError::Io(e) => write!(f, "I/O error: {}", e),
            CommonError::NotEnoughBytes(s) => write!(f, "Not enough bytes: {}", s),
            CommonError::ConversionFromBytes(e) => write!(f, "Conversion error: {}", e),
            CommonError::AddrParseError(e) => write!(f, "Address parsing error: {}", e),
            CommonError::Infallible(e) => write!(f, "Infallible error: {}", e),
            CommonError::Lock => write!(f, "Lock poisoned"),
            CommonError::Dns(e) => write!(f, "DNS error: {}", e),
            CommonError::ValidationError(e) => {
                write!(f, "Failed to validate: {}", e)
            }
            CommonError::SendError(e) => {
                write!(f, "Failed to send: {}", e)
            }
            CommonError::TryRecvError(e) => write!(f, "Failed to receive: {}", e),
            CommonError::IterError(e) => {
                write!(f, "Failed to iterate: {}", e)
            }
            CommonError::Generic(e) => write!(f, "We've entered uncharted waters: {}", e),
            CommonError::SocketCreateFailed(e) => {
                write!(f, "Failed to create Socket: {}", e)
            }
            CommonError::SocketConnectFailed(e) => {
                write!(f, "Failed to connect to address: {}", e)
            }
            CommonError::SocketBindFailed(e) => {
                write!(f, "Failed to bind Socket to provided address: {}", e)
            }
            CommonError::SocketListenFailed(e) => {
                write!(f, "Failed to call listen on socket: {}", e)
            }
            CommonError::SocketAcceptFailed(e) => {
                write!(f, "Failed to accept TCP connection: {}", e)
            }
            CommonError::SocketGetPeerName(e) => {
                write!(f, "Failed to get peer socket address: {}", e)
            }
            CommonError::UnknownAddressFamily => write!(f, "Failed to match address family"),
        }
    }
}

impl Error for CommonError {}

impl From<std::io::Error> for CommonError {
    fn from(e: std::io::Error) -> Self {
        CommonError::Io(e)
    }
}

impl From<std::array::TryFromSliceError> for CommonError {
    fn from(e: std::array::TryFromSliceError) -> Self {
        CommonError::ConversionFromBytes(e)
    }
}

impl From<std::net::AddrParseError> for CommonError {
    fn from(e: std::net::AddrParseError) -> Self {
        CommonError::AddrParseError(e)
    }
}

impl From<std::convert::Infallible> for CommonError {
    fn from(e: std::convert::Infallible) -> Self {
        CommonError::Infallible(e)
    }
}

impl<T> From<PoisonError<RwLockWriteGuard<'_, Vec<T>>>> for CommonError {
    fn from(_: PoisonError<RwLockWriteGuard<'_, Vec<T>>>) -> Self {
        CommonError::Lock
    }
}

impl<T> From<TryLockError<T>> for CommonError {
    fn from(_: TryLockError<T>) -> Self {
        CommonError::Lock
    }
}

impl From<&str> for CommonError {
    fn from(s: &str) -> Self {
        CommonError::Generic(s.to_owned())
    }
}

impl From<String> for CommonError {
    fn from(s: String) -> Self {
        CommonError::Generic(s)
    }
}

impl From<Box<dyn std::error::Error>> for CommonError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        CommonError::Generic(e.to_string())
    }
}

impl From<std::sync::mpsc::TryRecvError> for CommonError {
    fn from(e: std::sync::mpsc::TryRecvError) -> Self {
        CommonError::TryRecvError(e.to_string())
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for CommonError {
    fn from(e: std::sync::mpsc::SendError<T>) -> Self {
        CommonError::SendError(e.to_string())
    }
}
