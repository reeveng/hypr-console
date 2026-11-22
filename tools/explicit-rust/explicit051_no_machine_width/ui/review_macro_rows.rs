// edition:2024
// aux-build:console_core_number_conversion.rs
// aux-build:archmage_macros.rs
extern crate console_core_number_conversion;
extern crate archmage_macros;
use archmage_macros::arcane;

#[arcane]
fn sample() {
    for i in 0..8 {
        let _first = usize::wrapping_mul(i, 4);
        let _last = usize::wrapping_add(i, 3);
    }
}

fn main() { sample(); }
