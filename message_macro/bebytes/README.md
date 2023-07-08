# BeBytes

BeBytes is a Rust library that provides the BeBytes trait and the BeBytes derive macro for serialization and deserialization of network structs. It allows you to convert your Rust structs into byte representations (serialization) and vice versa (deserialization) using big endian order.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
bebytes = "0.1"
```

Then, import the BeBytes trait and use it as a trait bound in your method signatures:

```rust
use bebytes::BeBytes;

fn serialize<T: BeBytes>(data: &T) -> Vec<u8> {
    // Implement serialization logic using the `to_be_bytes` method
    data.to_be_bytes()
}

fn deserialize<T: BeBytes>(bytes: &[u8]) -> Result<T, Box<dyn std::error::Error>> {
    // Implement deserialization logic using the `try_from_be_bytes` method
    let (data, _) = T::try_from_be_bytes(bytes)?;
    Ok(data)
}
```

You can also use the BeBytes derive macro to automatically generate serialization and deserialization methods for your structs:

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

fn write_as_bytes(data: impl BeBytes) -> Vec<u8> {
    // Implement serialization logic using the `to_be_bytes` method
    data.to_be_bytes()
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

The BeBytes derive macro will generate the following methods for your struct:

- `try_from_be_bytes(&[u8]) -> Result<(Self, usize), Box<dyn std::error::Error>>`: A method to convert a byte slice into an instance of your struct. It returns a Result containing the deserialized struct and the number of consumed bytes.
- `to_be_bytes(&self) -> Vec<u8>`: A method to convert the struct into a byte representation. It returns a `Vec<u8>` containing the serialized bytes.
- `field_size(&self) -> usize`: A method to calculate the size (in bytes) of the struct.

## Contribute

I'm doing this for fun, but all help is appreciated. Thanks

## License

This project is licensed under the [MIT License](https://mit-license.org/)
