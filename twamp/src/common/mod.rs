use common::{
    error::CommonError,
    message::{Message, PacketResults},
    time::{DateTime, NtpTimestamp},
};
use message_macro::BeBytes;

pub const CONST_PADDING: usize = 27;
/// Unauthenticated TWAMP message as defined
/// in [RFC4656 Section 4.1.2](https://www.rfc-editor.org/rfc/rfc4656#section-4.1.2)
#[derive(Debug, PartialEq, Eq, Clone)]
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
    fn to_bytes(&self) -> Vec<u8> {
        let mut encoded = Vec::new();

        let seq_num_bytes = self.sequence_number.to_be_bytes();
        let timestamp_bytes = self.timestamp.into_bytes(); // Assuming NtpTimestamp has to_be_bytes() method
        let error_estimate_bytes = self.error_estimate.to_be_bytes(); // Use the to_bytes() method for ErrorEstimate

        encoded.extend_from_slice(&seq_num_bytes);
        encoded.extend_from_slice(&timestamp_bytes);
        encoded.extend_from_slice(&error_estimate_bytes);
        encoded.extend_from_slice(&self.padding);

        encoded
    }

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
#[derive(Debug, PartialEq, Eq, Clone)]
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
    fn to_bytes(&self) -> Vec<u8> {
        let mut encoded = Vec::new();

        let reflector_seq_num_bytes = self.reflector_sequence_number.to_be_bytes();
        let timestamp_bytes = self.timestamp.into_bytes(); // Assuming NtpTimestamp has to_be_bytes() method
        let error_estimate_bytes = self.error_estimate.to_be_bytes(); // Use the to_bytes() method for ErrorEstimate
        let mbz1_bytes = self.mbz1.to_be_bytes();
        let receive_timestamp_bytes = self.receive_timestamp.into_bytes();
        let sender_seq_num_bytes = self.sender_sequence_number.to_be_bytes();
        let sender_timestamp_bytes = self.sender_timestamp.into_bytes();
        let sender_error_estimate_bytes = self.sender_error_estimate.to_be_bytes();
        let mbz2_bytes = self.mbz2.to_be_bytes();
        let sender_ttl_bytes = [self.sender_ttl];

        encoded.extend_from_slice(&reflector_seq_num_bytes);
        encoded.extend_from_slice(&timestamp_bytes);
        encoded.extend_from_slice(&error_estimate_bytes);
        encoded.extend_from_slice(&mbz1_bytes);
        encoded.extend_from_slice(&receive_timestamp_bytes);
        encoded.extend_from_slice(&sender_seq_num_bytes);
        encoded.extend_from_slice(&sender_timestamp_bytes);
        encoded.extend_from_slice(&sender_error_estimate_bytes);
        encoded.extend_from_slice(&mbz2_bytes);
        encoded.extend_from_slice(&sender_ttl_bytes);
        encoded.extend_from_slice(&self.padding);

        encoded
    }

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

// impl ErrorEstimate {
//     pub fn new(s_bit: bool, z_bit: bool, scale: u8, multiplier: u8) -> Result<Self, CommonError> {
//         if multiplier == 0 {
//             return Err(CommonError::NotEnoughBytes(
//                 "Multiplier cannot be zero".to_string(),
//             ));
//         }

//         Ok(Self {
//             s_bit,
//             z_bit,
//             scale,
//             multiplier,
//         })
//     }

//     pub fn try_from_be_bytes(bytes: &[u8]) -> Result<Self, CommonError> {
//         if bytes.len() != 2 {
//             return Err(CommonError::NotEnoughBytes(
//                 "Invalid byte length".to_string(),
//             ));
//         }

//         let first_byte = bytes[0];
//         let s_bit = (first_byte >> 7) & 0x01 == 1;
//         let z_bit = (first_byte >> 6) & 0x01 == 1;
//         let scale = first_byte & 0x3F;
//         let multiplier = bytes[1];

//         if multiplier == 0 {
//             log::error!("Multiplier cannot be zero");
//         }

//         Ok(Self {
//             s_bit,
//             z_bit,
//             scale,
//             multiplier,
//         })
//     }

//     pub fn to_bytes(&self) -> [u8; 2] {
//         let mut first_byte: u8 = 0;

//         if self.s_bit {
//             first_byte |= 1 << 7;
//         }

//         first_byte |= self.scale & 0x3F; // Keep only the 6 least significant bits

//         let encoded: [u8; 2] = [first_byte, self.multiplier];
//         encoded
//     }
// }
