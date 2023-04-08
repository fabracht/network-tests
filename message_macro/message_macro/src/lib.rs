pub use message_macro_derive::BeBytes;

pub trait BeBytes {
    fn to_be_bytes(&self) -> Vec<u8>;
    fn try_from_be_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;
}
