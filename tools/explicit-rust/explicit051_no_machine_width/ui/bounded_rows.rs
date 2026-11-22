// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

fn rows(buf: &[u8; 16]) -> u32 {
    let mut sum = 0;
    for j in 0..4 {
        sum += u32::from(buf[usize::wrapping_mul(j, 4)]);
        sum += u32::from(buf[usize::wrapping_mul(j, 4).wrapping_add(3)]);
    }
    sum
}

fn inclusive() -> [bool; 2] {
    let mut reversed = true;
    for i in 4..=3 {
        reversed &= usize::wrapping_add(i, 1) == 0;
        reversed &= usize::wrapping_mul(i, 2) == 0;
    }
    let mut visited = false;
    for i in 65535..=65535 {
        visited = usize::wrapping_sub(i, 65535) == 0 && usize::wrapping_mul(i, 0) == 0;
    }
    [reversed, visited]
}

fn main() {
    assert_eq!(rows(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]), 60);
    assert_eq!(inclusive(), [true, true]);
}
