use message_macro::BeBytes;
fn main() {
    // let error_estimate = ErrorEstimate {
    //     s_bit: 1,
    //     z_bit: 0,
    //     scale: 63,
    //     dummy_struct: DummyStruct {
    //         dummy1: 1,
    //         dummy2: 2,
    //     },
    //     padding: vec![1; 27],
    // };
    // let bytes = error_estimate.to_be_bytes();
    // println!("Bytes len: {}", bytes.len());
    // for byte in &bytes {
    //     print!("{:08b} ", byte);
    // }
    // let error = ErrorEstimate::try_from_be_bytes(&bytes);
    // println!("\nError: {:?}", error);
    let dummy = DummyStruct {
        dummy0: [0, 2],
        dummy1: 1,
        dummy2: 2,
    };
    let dummy_bytes = dummy.to_be_bytes();

    let dummy_error = DummyStruct::try_from_be_bytes(&dummy_bytes);
    println!("\ndummy error {:?}", dummy_error);
}

#[derive(BeBytes, Debug, PartialEq, Clone)]
pub struct DummyStruct {
    pub dummy0: [u8; 2],
    pub dummy1: u32,
    pub dummy2: u32,
}

// #[derive(BeBytes, Debug, PartialEq)]
// pub struct ErrorEstimate {
//     #[U8(size(1), pos(0))]
//     pub s_bit: u8,
//     #[U8(size(1), pos(1))]
//     pub z_bit: u8,
//     #[U8(size(6), pos(2))]
//     pub scale: u8,
//     pub dummy_struct: DummyStruct,
//     pub padding: Vec<u8>,
// }
