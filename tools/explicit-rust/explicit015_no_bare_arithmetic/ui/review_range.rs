#![allow(dead_code, unused_assignments)]

fn review(n: u8) {
    let mut step = 1_u8;
    step = n;
    std::hint::black_box(step + 1);
    for mut x in 0_u8..10 {
        x = n;
        std::hint::black_box(x + 1);
    }
    for x in 0..=255_u8 {
        std::hint::black_box(x + 1);
    }
    for x in 0..n {
        std::hint::black_box(x * 2);
    }
    for x in 0..=n {
        std::hint::black_box(x + 1);
    }
    for x in 0..3_u8 {
        let x = std::hint::black_box(n.max(x));
        std::hint::black_box(x * 2);
    }
}
struct Mask;
impl std::ops::BitAnd<u8> for Mask {
    type Output = u8;
    fn bitand(self, _: u8) -> u8 { u8::MAX }
}
fn overloaded() {
    let value = Mask & 3;
    std::hint::black_box(value + 1);
}

const LIMIT: u8 = if cfg!(any()) { u8::MAX } else { 14 };
fn configured(value: u8) -> u8 { value.min(LIMIT) + 1 }

fn main() {}
