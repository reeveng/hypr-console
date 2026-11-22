// edition:2024
#![allow(dead_code)]

fn different_place(guarded: &[u8], used: &[u8], at: usize) -> bool {
    at < guarded.len() && used[at] == b'x'
}

fn different_offset(bytes: &[u8], at: usize) -> bool {
    at < bytes.len() && bytes[at + 1] == b'x'
}

fn insufficient_guard(bytes: &[u8]) -> bool {
    bytes.len() > 1 && bytes[2] == b'x'
}

fn effectful_offset(bytes: &[u8], at: impl Fn() -> usize) -> bool {
    at() < bytes.len() && bytes[at()] == b'x'
}

fn borrowed_argument(bytes: &[u8], at: usize, check: fn(&u8) -> bool) -> bool {
    at < bytes.len() && check(&bytes[at])
}

fn has_let_condition(bytes: &[u8], position: Option<usize>) -> bool {
    if let Some(at) = position && at < bytes.len() && bytes[at] == b'x' && at == 0 {
        return true;
    }
    false
}

fn main() {}
