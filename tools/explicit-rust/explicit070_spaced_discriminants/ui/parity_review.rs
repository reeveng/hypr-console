#[repr(u8)]
enum FullByte {
    First = 0,
    Last = 128,
}

#[repr(u16)]
enum Unselected {
    First = 0,
    Next = 1,
}

fn main() {
    assert_eq!(FullByte::First as u8, 0);
    assert_eq!(FullByte::Last as u8, 128);
    assert_eq!(Unselected::First as u16, 0);
    assert_eq!(Unselected::Next as u16, 1);
}
