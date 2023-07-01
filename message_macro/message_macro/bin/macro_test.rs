use message_macro::BeBytes;
fn main() {
    let error_estimate = ErrorEstimateMini {
        s_bit: 1,
        z_bit: 0,
        scale: 63,
        multiplier: 3,
    };
    let bytes = error_estimate.to_be_bytes();
    println!("Bytes len: {}", bytes.len());
    for byte in &bytes {
        print!("{:08b} ", byte);
    }
    let error = ErrorEstimateMini::try_from_be_bytes(&bytes);
    println!("\nError: {:?}", error);
    assert_eq!(error_estimate, error.unwrap().0);
    let error_estimate = ErrorEstimate {
        s_bit: 1,
        z_bit: 0,
        scale: 64,
        dummy_struct: DummyStruct {
            dummy1: 1,
            dummy2: 2,
            dummy0: [3, 4],
        },
        padding: vec![1; 27],
    };
    let bytes = error_estimate.to_be_bytes();
    println!("Bytes len: {}", bytes.len());
    for byte in &bytes {
        print!("{:08b} ", byte);
    }
    let error = ErrorEstimate::try_from_be_bytes(&bytes);
    println!("\nError: {:?}", error);
    assert_eq!(error_estimate, error.unwrap().0);

    let dummy = DummyStruct {
        dummy0: [0, 2],
        dummy1: 1,
        dummy2: 2,
    };
    let dummy_bytes = dummy.to_be_bytes();

    let re_dummy = DummyStruct::try_from_be_bytes(&dummy_bytes);
    println!("\ndummy error {:?}", re_dummy);
    assert_eq!(dummy, re_dummy.unwrap().0);

    let dummy_enum = DummyEnum::ServerStart;
    let dummy_enum_bytes = dummy_enum.to_be_bytes();
    println!("{:?}", dummy_enum_bytes);
    let re_dummy_enum = DummyEnum::try_from_be_bytes(&dummy_enum_bytes);
    println!("{:?}", re_dummy_enum);
    assert_eq!(dummy_enum, re_dummy_enum.unwrap().0);

    let u_8 = U8 {
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
    };
    let u_8_bytes = u_8.to_be_bytes();
    println!("{:?}", u_8_bytes);
    let re_u_8 = U8::try_from_be_bytes(&u_8_bytes);
    println!("{:?}", re_u_8);
    assert_eq!(u_8, re_u_8.unwrap().0);

    let u_16 = U16 {
        first: 1,
        second: 16383,
        fourth: 0,
    };
    let u_16_bytes = u_16.to_be_bytes();

    println!("{:?}", u_16_bytes);
    let re_u_16 = U16::try_from_be_bytes(&u_16_bytes);
    println!("{:?}", re_u_16);
    assert_eq!(u_16, re_u_16.unwrap().0);

    let u_32 = U32 {
        first: 1,
        second: 32383,
        fourth: 1,
    };
    let u_32_bytes = u_32.to_be_bytes();

    println!("{:?}", u_32_bytes);
    let re_u_32 = U32::try_from_be_bytes(&u_32_bytes);
    println!("{:?}", re_u_32);
    assert_eq!(u_32, re_u_32.unwrap().0);
}

#[derive(BeBytes, Debug, PartialEq)]
struct U8 {
    #[U8(size(1), pos(0))]
    first: u8,
    #[U8(size(3), pos(1))]
    second: u8,
    #[U8(size(4), pos(4))]
    third: u8,
    fourth: u8,
}

#[derive(BeBytes, Debug, PartialEq)]
struct U16 {
    #[U8(size(1), pos(0))]
    first: u8,
    #[U8(size(14), pos(1))]
    second: u16,
    #[U8(size(1), pos(15))]
    fourth: u8,
}

#[derive(BeBytes, Debug, PartialEq)]
struct U32 {
    #[U8(size(1), pos(0))]
    first: u8,
    #[U8(size(30), pos(1))]
    second: u32,
    #[U8(size(1), pos(31))]
    fourth: u8,
}

#[derive(BeBytes, Debug, PartialEq)]
pub enum DummyEnum {
    SetupResponse = 1,
    ServerStart = 2,
    SetupRequest = 3,
}

#[derive(BeBytes, Debug, PartialEq, Clone)]
pub struct DummyStruct {
    pub dummy0: [u8; 2],
    pub dummy1: u32,
    pub dummy2: u32,
}

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

#[derive(BeBytes, Debug, PartialEq)]
pub struct ErrorEstimateMini {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    pub multiplier: u32,
}
