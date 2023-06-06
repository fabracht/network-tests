#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use message_macro::BeBytes;
fn main() {
    let error_estimate = ErrorEstimateMini {
        s_bit: 1,
        z_bit: 0,
        scale: 65,
        multiplier: 3,
    };
    let bytes = error_estimate.to_be_bytes();
    {
        ::std::io::_print(format_args!("Bytes len: {0}\n", bytes.len()));
    };
    for byte in &bytes {
        {
            ::std::io::_print(format_args!("{0:08b} ", byte));
        };
    }
    let error = ErrorEstimateMini::try_from_be_bytes(&bytes);
    {
        ::std::io::_print(format_args!("\nError: {0:?}\n", error));
    };
    match (&error_estimate, &error.unwrap().0) {
        (left_val, right_val) => {
            if !(*left_val == *right_val) {
                let kind = ::core::panicking::AssertKind::Eq;
                ::core::panicking::assert_failed(
                    kind,
                    &*left_val,
                    &*right_val,
                    ::core::option::Option::None,
                );
            }
        }
    };
}
pub struct ErrorEstimateMini {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    pub multiplier: u32,
}
impl BeBytes for ErrorEstimateMini {
    fn try_from_be_bytes(
        bytes: &[u8],
    ) -> Result<(Self, usize), Box<dyn std::error::Error>> {
        let mut bit_sum = 0;
        let mut byte_index = 0;
        let mut end_byte_index = 0;
        bit_sum += 1usize;
        let shift_factor = 8 - 0usize % 8;
        let s_bit = ((bytes[0usize] as u8) >> (7 - (1usize + 0usize % 8 - 1) as u8))
            & (1i32 as u8);
        bit_sum += 1usize;
        let shift_factor = 8 - 1usize % 8;
        let z_bit = ((bytes[0usize] as u8) >> (7 - (1usize + 1usize % 8 - 1) as u8))
            & (1i32 as u8);
        bit_sum += 6usize;
        let shift_factor = 8 - 2usize % 8;
        let scale = ((bytes[0usize] as u8) >> (7 - (6usize + 2usize % 8 - 1) as u8))
            & (63i32 as u8);
        byte_index = bit_sum / 8;
        end_byte_index = byte_index + 4usize;
        bit_sum += 8 * 4usize;
        let multiplier = <u32>::from_be_bytes({
            let slice = &bytes[byte_index..end_byte_index];
            let mut arr = [0; 4usize];
            arr.copy_from_slice(slice);
            arr
        });
        Ok((
            Self {
                s_bit: s_bit,
                z_bit: z_bit,
                scale: scale,
                multiplier: multiplier,
            },
            bit_sum / 8,
        ))
    }
    fn to_be_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let s_bit = self.s_bit.to_owned();
            if (s_bit as u8) & !(1i32 as u8) != 0 {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "Value {0} for field {1} exceeds the maximum allowed value {2}.",
                        s_bit, "s_bit", 1i32
                    ),
                );
            }
            if bytes.len() <= 0usize {
                bytes.resize(0usize + 1, 0);
            }
            bytes[0usize] |= (s_bit as u8) << (7 - (1usize - 1) - 0usize % 8);
        }
        {
            let z_bit = self.z_bit.to_owned();
            if (z_bit as u8) & !(1i32 as u8) != 0 {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "Value {0} for field {1} exceeds the maximum allowed value {2}.",
                        z_bit, "z_bit", 1i32
                    ),
                );
            }
            if bytes.len() <= 0usize {
                bytes.resize(0usize + 1, 0);
            }
            bytes[0usize] |= (z_bit as u8) << (7 - (1usize - 1) - 1usize % 8);
        }
        {
            let scale = self.scale.to_owned();
            if (scale as u8) & !(63i32 as u8) != 0 {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "Value {0} for field {1} exceeds the maximum allowed value {2}.",
                        scale, "scale", 63i32
                    ),
                );
            }
            if bytes.len() <= 0usize {
                bytes.resize(0usize + 1, 0);
            }
            bytes[0usize] |= (scale as u8) << (7 - (6usize - 1) - 2usize % 8);
        }
        {
            let multiplier = self.multiplier.to_owned();
            bytes.extend_from_slice(&multiplier.to_be_bytes());
        }
        bytes
    }
    fn field_size(&self) -> usize {
        std::mem::size_of_val(self)
    }
}
impl ErrorEstimateMini {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        s_bit: u8,
        z_bit: u8,
        scale: u8,
        multiplier: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            s_bit: s_bit,
            z_bit: z_bit,
            scale: scale,
            multiplier: multiplier,
        })
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for ErrorEstimateMini {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field4_finish(
            f,
            "ErrorEstimateMini",
            "s_bit",
            &self.s_bit,
            "z_bit",
            &self.z_bit,
            "scale",
            &self.scale,
            "multiplier",
            &&self.multiplier,
        )
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for ErrorEstimateMini {}
#[automatically_derived]
impl ::core::cmp::PartialEq for ErrorEstimateMini {
    #[inline]
    fn eq(&self, other: &ErrorEstimateMini) -> bool {
        self.s_bit == other.s_bit && self.z_bit == other.z_bit
            && self.scale == other.scale && self.multiplier == other.multiplier
    }
}
