// run-rustfix
// edition:2024
#![allow(unused_parens)]

fn neighbors(end: u8) {
    for x in 1..end {
        assert!(x + 1 <= end);
        assert_eq!(x / x, 1);
    }
}

fn main() {
    neighbors(u8::MAX);
    for y in 0_usize..16 {
        assert!((y * 16) < 256);
        let row_offset = y.wrapping_mul(16);
        let last = row_offset + 15;
        assert!(last <= 255);
        for y in 1_u8..=3 {
            assert!(12 / y >= 4);
        }
    }
    for x in 0..=255_u8 {
        assert!(u32::from(x) + 1 <= 256);
    }
}
