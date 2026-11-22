// edition:2024
fn advance(item: Option<u8>, use_item: bool) -> Option<u8> {
    let read = match use_item { true => Some(item?), false => Some(0) };
    Some(read.unwrap_or(1) + 1)
}
fn main() { assert_eq!(advance(None, true), None); }
