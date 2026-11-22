// edition:2024
macro_rules! explained { ($code:expr) => { ($code, stringify!($code)) } }
fn documented(input: i32) -> (Option<u8>, &'static str) {
    explained!(u8::try_from(input).ok())
}

fn main() {
    assert_eq!(documented(-1), (None, "u8::try_from(input).ok()"));
}
