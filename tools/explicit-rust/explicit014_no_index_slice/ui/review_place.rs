#![allow(dead_code, unconditional_panic)]
fn keep(a: &mut [u8; 2], i: usize, slice: &[u8]) {
    a[0] = 2;
    let borrow = &mut a[1];
    *borrow = 4;
    let dynamic = a[i];
    let unproven = slice[0];
    let exceeds = a[2];
    let bounded_end = &a[..1];
    const OFFSET: usize = 1;
    let configured = a[OFFSET];
    let effectful = a[{ std::hint::black_box(()); 0 }];
    std::hint::black_box((dynamic, unproven, exceeds, bounded_end, configured, effectful));
}
fn main() {}
const TARGET_LENGTH: usize = 32 + size_of::<usize>();

fn target_length(values: &[u8; TARGET_LENGTH]) -> (&u8, &[u8]) {
    (&values[32], &values[..33])
}

fn configured_length() -> u8 {
    #[cfg(target_pointer_width = "64")]
    let values = [0u8; 40];
    #[cfg(not(target_pointer_width = "64"))]
    let values = [0u8; 36];
    values[32]
}

fn inside_loop() -> u8 {
    let mut sum = 0;
    for index in 0..2 {
        let values = [3u8, 4];
        sum += values[index];
    }
    sum
}
