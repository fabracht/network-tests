pub use bebytes_derive::BeBytes;

pub trait BeBytes {
    fn field_size(&self) -> usize;
    fn to_be_bytes(&self) -> Vec<u8>;
    fn try_from_be_bytes(bytes: &[u8]) -> Result<(Self, usize), Box<dyn std::error::Error>>
    where
        Self: Sized;
}
