use message_macro::BeBytes;
fn main() {
    // let error_estimate = ErrorEstimate::new(
    //     1,
    //     0,
    //     31,
    //     1,
    //     1,
    //     0,
    //     31,
    //     240,
    //     Some(13),
    //     DummyStruct {},
    //     vec![1; 1],
    // )
    // .unwrap();
    // let error_estimate = ErrorEstimate {
    //     dummy_struct: DummyStruct { dummy: 1 },
    // };
    let error_estimate = ErrorEstimate {
        s_bit: 1,
        z_bit: 0,
        scale: 63,
        scale2: 1,
        dummy_option: Some(1),
        dummy_struct: DummyStruct { dummy: 2 },
        scale3: 3,
        padding: vec![1; 2],
    };
    let bytes = error_estimate.to_be_bytes();
    for byte in &bytes {
        print!("{:08b} ", byte);
    }
    let error = ErrorEstimate::try_from_be_bytes(&bytes);
    let dummy = DummyStruct { dummy: 1 };
    let dummy_bytes = dummy.to_be_bytes();

    let dummy_error = DummyStruct::try_from_be_bytes(&dummy_bytes);
    println!("\ndummy error {:?}", dummy_error);
    println!("\nError: {:?}", error);
}

#[derive(BeBytes, Debug, PartialEq, Clone)]
pub struct DummyStruct {
    pub dummy: u8,
}

#[derive(BeBytes, Debug, PartialEq)]
pub struct ErrorEstimate {
    #[U8(size(1), pos(0))]
    pub s_bit: u8,
    #[U8(size(1), pos(1))]
    pub z_bit: u8,
    #[U8(size(6), pos(2))]
    pub scale: u8,
    // pub pre_padding: u8,
    // #[U8(size(1), pos(0))]
    // pub s_bit1: u8,
    // #[U8(size(2), pos(1))]
    // pub z_bit1: u8,
    // #[U8(size(5), pos(3))]
    // pub scale1: u8,
    pub scale2: u16,
    pub dummy_option: Option<u16>,
    pub dummy_struct: DummyStruct,
    pub scale3: u16,
    pub padding: Vec<u8>,
}
