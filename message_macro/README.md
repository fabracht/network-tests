# Custom Attributes

`custom_attributes` is a Rust procedural macro that allows you to define custom bit-level attributes for your structs. It supports signed integers (i8, i16, i32, i64, i128), unsigned integers (u8, u16, u32, u64, u128), and floating-point numbers (f32, f64).

## Usage

Add `custom_attributes` to your `Cargo.toml`:

```toml
[dependencies]
message_macro = "0.1.0"
```

In your Rust file, import and use the `BeBytes` macro:

```rust
use message_macro::BeBytes;

#[derive(BeBytes, Debug)]
pub struct MyStruct {
    // Your fields go here, using the U8 attribute as needed.
}
```

## Example

```rust
use message_macro::BeBytes;

#[derive(BeBytes, Debug)]
pub struct ErrorEstimate {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    pub multiplier: u8,
}
```

## Supported Types

The following types are supported:

- i8, i16, i32, i64, i128
- u8, u16, u32, u64, u128
- f32, f64
- Vec<T> where T is one of the types above
- Option<T>

The U8 attribute is only applicable to the u8 type. Using it with other types will result in a compilation error.
