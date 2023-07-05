# BeBytes Derive

BeBytes Derive is a procedural macro crate that provides a custom derive macro for generating serialization and deserialization methods for network structs in Rust. The macro generates code to convert the struct into a byte representation (serialization) and vice versa (deserialization) using big endian order. It aims to simplify the process of working with network protocols and message formats by automating the conversion between Rust structs and byte arrays.

**Note: BeBytes Derive is currently in development and has not been thoroughly tested in production environments. Use it with caution and ensure proper testing and validation in your specific use case.**

## Usage

To use BeBytes Derive, add it as a dependency in your `Cargo.toml` file:

```toml
[dependencies]
bebytes_derive = "0.2"
```

Then, import the BeBytes trait from the bebytes_derive crate and derive it for your struct:

```rust
use bebytes_derive::BeBytes;

#[derive(BeBytes)]
struct MyStruct {
    // Define your struct fields here...
}
```

The BeBytes derive macro will generate the following methods for your struct:

- `try_from_be_bytes(&[u8]) -> Result<(Self, usize), Box<dyn std::error::Error>>`: A method to convert a byte slice into an instance of your struct. It returns a Result containing the deserialized struct and the number of consumed bytes.
- `to_be_bytes(&self) -> Vec<u8>`: A method to convert the struct into a byte representation. It returns a `Vec<u8>` containing the serialized bytes.
- `field_size(&self) -> usize`: A method to calculate the size (in bytes) of the struct.

## Example

Here's an example showcasing the usage of the BeBytes Derive:

```rust
use message_macro_derive::BeBytes;

#[derive(Debug, BeBytes)]
struct MyStruct {
    #[U8(size(1), pos(0))]
    field1: u8,
    #[U8(size(4), pos(1))]
    field2: u8,
    #[U8(size(3), pos(5))]
    field3: u8,
    field4: u32,
}

fn main() {
    let my_struct = MyStruct {
        field1: 1,
        field2: 7,
        field3: 12,
        field4: 0
    };

    let bytes = my_struct.to_be_bytes();
    println!("Serialized bytes: {:?}", bytes);

    let deserialized = MyStruct::try_from_be_bytes(&bytes).unwrap();
    println!("Deserialized struct: {:?}", deserialized);
}
```

In this example, we define a struct MyStruct with four fields. The `#[U8]` attribute is used to specify the size and position of the fields for serialization. The BeBytes derive macro generates the serialization and deserialization methods for the struct, allowing us to easily convert it to bytes and back.

## How it works

The `U8` attribute allows you to define 2 attributes, `pos` and `size`. The position attribute defines the position in current byte where the bits should start. For example, a pos(0), size(4) specifies that the field should take only 4 bits and should start at position 0 from left to right. The macro will displace the bits so that they occupy the correct place in the resulting byte vector when `.to_be_bytes()` is used. So a `4` with pos(0) and size(4):

4 => 00000100 
Shifted and masked => 0100

Fields are read/written sequentially in Big Endian order and MUST complete a multiple of 8.
This means that fields decorated with the `U8` attribute MUST complete a byte before the next non `U8` byte is provided. For example, the struct 

```rust
#[derive(Debug, BeBytes)]
struct WrongStruct {
    #[U8(size(1), pos(0))]
    field1: u8,
    #[U8(size(4), pos(1))]
    field2: u8,
    field3: f32,
}
```

will through a compile time error saying that a `U8` attribute must add up to the full byte.

As long as you follow the above rule, you can create custom sequence of bits by using Rust unsigned integers as types and the derived implementation will take care of the nasty shifting and masking for you.
One of the advantages is that we don't need an intermediate vector implementation to parse groups of or individual bits.

## Multi Byte values
The macro has support for all unsigned types from u8 to u128. These can be used in the same way the u8 type is used:
- Using a u16
```rust
#[derive(BeBytes, Debug, PartialEq)]
struct U16 {
    #[U8(size(1), pos(0))]
    first: u8,
    #[U8(size(14), pos(1))]
    second: u16,
    #[U8(size(1), pos(15))]
    fourth: u8,
}
```

- Using a u32
```rust
#[derive(BeBytes, Debug, PartialEq)]
struct U32 {
    #[U8(size(1), pos(0))]
    first: u8,
    #[U8(size(30), pos(1))]
    second: u32,
    #[U8(size(1), pos(31))]
    fourth: u8,
}
```

And so on.

**The same rules apply here. Your `U8` fields must complete a byte, even if they span over multiple bytes.**

## Enums

Only enums with named fields are supported and values are read/written as a byte.
Example:

```rust
#[derive(BeBytes, Debug, PartialEq)]
pub enum DummyEnum {
    SetupResponse = 1,
    ServerStart = 2,
    SetupRequest = 3,
}
```

## Options

Options are supported, as long as the internal type is a primitive
Example:

```rust
#[derive(BeBytes, Debug, PartialEq)]
pub struct NestedStruct {
    pub dummy_struct: DummyStruct,
    pub optional_number: Option<i32>,
    pub error_estimate: ErrorEstimate,
}
```


## Byte arrays and Vectors

You can pass a static array of bytes, since the size if known at compilation time.
Example:
```rust
pub struct DummyStruct {
    pub dummy0: [u8; 2],
    #[U8(size(1), pos(0))]
    pub dummy1: u8,
    #[U8(size(7), pos(1))]
    pub dummy2: u8,
}
```
 
Vectors can ONLY be used as the last field.

Example:
```rust
#[derive(BeBytes, Debug, PartialEq)]
pub struct ErrorEstimate {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    pub dummy_struct: DummyStruct,
    pub padding: Vec<u8>,
}
```

Trying to place a vector anywhere else in the sequence produces a compile time error.

## Nested Fields

In theory, you can nest structures, but beware of padding vectors. I have not implemented, nor tested anything to prevent you from doing it, so just don't put nested structs with vectors in it unless they are occupy the last position.

```rust
#[derive(BeBytes, Debug, PartialEq)]
pub struct NestedStruct {
    pub dummy_struct: DummyStruct,
    pub error_estimate: ErrorEstimate,
}
```

## Contribute
I'm doing this for fun, but all help is appreciated. Thanks

## License

This project is licensed under the [MIT License](https://mit-license.org/)
