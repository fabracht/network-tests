use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::sync::{PoisonError, RwLockWriteGuard};

pub type Result<T> = std::result::Result<T, CommonError>;
#[derive(Debug)]
pub enum CommonError {
    Io(std::io::Error),
    NotEnoughBytes(String),
    ConversionFromBytes(std::array::TryFromSliceError),
    AddrParseError(std::net::AddrParseError),
    Infallible(std::convert::Infallible),
    Lock,
    Dns(String),
    KeventRegistrationError(std::io::Error), // Added new error variant
    ValidationError(validator::ValidationErrors),
    SendError(String),
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
            CommonError::KeventRegistrationError(e) => {
                write!(f, "Kevent registration error: {}", e)
            }
            CommonError::ValidationError(e) => {
                write!(f, "Failed to validate: {}", e)
            }
            CommonError::SendError(e) => {
                write!(f, "Failed to send: {}", e)
            }
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

impl From<&str> for CommonError {
    fn from(s: &str) -> Self {
        CommonError::Dns(s.to_owned())
    }
}

impl From<String> for CommonError {
    fn from(s: String) -> Self {
        CommonError::Dns(s)
    }
}

impl From<Box<dyn std::error::Error>> for CommonError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        CommonError::Dns(e.to_string())
    }
}
