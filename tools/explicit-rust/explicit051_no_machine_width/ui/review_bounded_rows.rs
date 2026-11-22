// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

macro_rules! spelling { ($e:expr) => { (stringify!($e), $e) } }

fn main() {
    for i in 0..65536 {
        let _first = usize::wrapping_mul(i, 4);
        let _last = usize::wrapping_add(i, 7);
    }
    for i in 0..4 {
        let _first = spelling!(usize::wrapping_mul(i, 4));
        let _last = usize::wrapping_add(i, 7);
    }
    for i in 0..4 {
        #[cfg(any())]
        let _: i32 = i;
        let _first = usize::wrapping_mul(i, 4);
        let _last = usize::wrapping_add(i, 7);
    }
}
