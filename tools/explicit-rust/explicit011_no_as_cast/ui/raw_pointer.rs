// run-rustfix
// edition:2024
#![allow(dead_code)]

fn shared(from: *const u32) -> *const u8 {
    from as *const u8
}

fn writable(from: *mut u16) -> *mut u32 {
    from as *mut u32
}

fn main() {}
