// edition:2024
macro_rules! inspect {
    ($value:expr) => {{
        let _ = stringify!($value);
        core::matches!($value, 3)
    }};
}

fn forwarded(read: impl FnOnce() -> Option<u8>) -> Option<bool> {
    Some(inspect!(read()?))
}

fn borrowed(read: impl FnOnce() -> Option<u8>) -> Option<bool> {
    Some(matches!(read()?, ref byte if *byte == 3))
}

fn temporary(read: &std::cell::RefCell<u8>) -> Option<bool> {
    Some(matches!(Some(*read.borrow())?, 3))
}

fn main() {}
