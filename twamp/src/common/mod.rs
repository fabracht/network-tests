use common::{
    error::CommonError,
    message::{Message, PacketResults},
    time::{DateTime, NtpTimestamp},
};
use message_macro::BeBytes;

pub const CONST_PADDING: usize = 27;

/// Unauthenticated TWAMP message as defined
/// in [RFC4656 Section 4.1.2](https://www.rfc-editor.org/rfc/rfc4656#section-4.1.2)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone)]
#[repr(C)]
pub struct SenderMessage {
    /// Sender sequence number
    pub sequence_number: u32,
    /// Timestamp
    pub timestamp: NtpTimestamp,
    /// Error estimate on timestamp
    pub error_estimate: ErrorEstimate,
    /// Payload of the packet to send
    pub padding: Vec<u8>,
}

impl TryFrom<&[u8]> for SenderMessage {
    type Error = CommonError;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        let sequence_number = u32::from_be_bytes(buffer[0..4].try_into()?);
        let timestamp = NtpTimestamp::try_from_be_bytes(buffer[4..12].try_into()?)?;
        let error_estimate = ErrorEstimate::try_from_be_bytes(&mut [buffer[12], buffer[13]])?;
        let mut padding = Vec::new();
        padding.resize(CONST_PADDING, 0);
        Ok(Self {
            sequence_number,
            timestamp,
            error_estimate,
            padding,
        })
    }
}

impl Message for SenderMessage {
    fn packet_results(&self) -> PacketResults {
        PacketResults {
            sender_seq: self.sequence_number,
            reflector_seq: None,
            t1: DateTime::try_from(self.timestamp).unwrap(),
            t2: None,
            t3: None,
            t4: None,
        }
    }
}

/// Unauthenticated TWAMP message as defined
/// in [RFC5357 Section 4.2.1](https://www.rfc-editor.org/rfc/rfc5357.html#section-4.2.1)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone)]
#[repr(C)]
pub struct ReflectedMessage {
    /// Reflector sequence number
    pub reflector_sequence_number: u32,
    /// Timestamp
    pub timestamp: NtpTimestamp,
    /// Error estimate on the timestamp
    pub error_estimate: ErrorEstimate,
    /// Must be zero
    pub mbz1: u16,
    /// Receive timestamp
    pub receive_timestamp: NtpTimestamp,
    /// Sender sequence number
    pub sender_sequence_number: u32,
    /// Timestamp
    pub sender_timestamp: NtpTimestamp,
    /// Error estimate on timestamp
    pub sender_error_estimate: ErrorEstimate,
    /// Must be zero
    pub mbz2: u16,
    /// Time to live (TTL) field of the sender's IP header
    pub sender_ttl: u8,
    /// Payload of the packet to send
    pub padding: Vec<u8>,
}

impl TryFrom<&[u8]> for ReflectedMessage {
    type Error = CommonError;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        if buffer.len() < 41 {
            return Err(CommonError::NotEnoughBytes(format!("{}", buffer.len())));
        }
        let sequence_number = u32::from_be_bytes(buffer[0..4].try_into()?);
        let timestamp = NtpTimestamp::try_from_be_bytes(buffer[4..12].try_into()?)?;
        let error_estimate = ErrorEstimate::try_from_be_bytes(&mut [buffer[12], buffer[13]])?;
        let receive_timestamp = NtpTimestamp::try_from_be_bytes(buffer[16..24].try_into()?)?;
        let sender_sequence_number = u32::from_be_bytes(buffer[24..28].try_into()?);

        let sender_timestamp = NtpTimestamp::try_from_be_bytes(buffer[28..36].try_into()?)?;
        let sender_error_estimate =
            ErrorEstimate::try_from_be_bytes(&mut [buffer[36], buffer[37]])?;

        let sender_ttl = buffer[40];
        Ok(ReflectedMessage {
            reflector_sequence_number: sequence_number,
            timestamp,
            error_estimate,
            mbz1: 0,
            receive_timestamp,
            sender_sequence_number,
            sender_timestamp,
            sender_error_estimate,
            mbz2: 0,
            sender_ttl,
            padding: buffer[41..].to_vec(),
        })
    }
}

impl Message for ReflectedMessage {
    fn packet_results(&self) -> PacketResults {
        PacketResults {
            sender_seq: self.sender_sequence_number,
            reflector_seq: Some(self.reflector_sequence_number),
            t1: DateTime::try_from(self.sender_timestamp).unwrap(),
            t2: DateTime::try_from(self.receive_timestamp).ok(),
            t3: DateTime::try_from(self.timestamp).ok(),
            t4: None,
        }
    }
}

/// Estimation on the error on a timestamp based
/// on synchronization method used [RFC4656 Section 4.1.2](https://www.rfc-editor.org/rfc/rfc4656#section-4.1.2)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone, Copy)]
pub struct ErrorEstimate {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    pub multiplier: u8,
}
