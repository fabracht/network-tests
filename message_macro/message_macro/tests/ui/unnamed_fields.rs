use message_macro::BeBytes;

#[derive(BeBytes)]
enum UnsupportedEnum {
    A,
    B,
}

fn main() {}
