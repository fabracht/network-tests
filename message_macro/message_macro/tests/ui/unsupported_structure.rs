use message_macro::BeBytes;

#[derive(BeBytes)]
struct UnsupportedStruct(u8, u16, u32);

fn main() {}
