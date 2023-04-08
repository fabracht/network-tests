use message_macro::CustomAttributes;

#[derive(CustomAttributes)]
struct UnsupportedStruct(u8, u16, u32);

fn main() {}
