// run-rustfix
// edition:2024
// rustc-env:CARGO_PKG_RUST_VERSION=1.88
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

fn update(state: &mut [u8; 4]) {
    for row in 0..2 {
        let Ok(saved) = ::console_core_number_conversion::fixed_index(usize::wrapping_mul(row, 2));
        let [left, right] = [
            state[match ::console_core_number_conversion::index(saved) { Ok(native) => native }],
            state[(match ::console_core_number_conversion::index(saved) {
                Ok(native) => native,
            }).wrapping_add(1)],
        ];
        state[match ::console_core_number_conversion::index(saved) { Ok(native) => native }] = right;
        state[(match ::console_core_number_conversion::index(saved) {
            Ok(native) => native,
        }).wrapping_add(1)] = left;
    }
}

fn use_position(state: &mut [u8; 4]) -> u64 {
    let mut observed = 0;
    for row in 0..2 {
        let Ok(saved) = ::console_core_number_conversion::fixed_index(usize::wrapping_mul(row, 2));
        state[match ::console_core_number_conversion::index(saved) { Ok(native) => native }] = 42;
        observed += saved;
    }
    observed
}

fn main() {
    let mut state = [1, 2, 3, 4];
    update(&mut state);
    assert_eq!(state, [2, 1, 4, 3]);
    assert_eq!(use_position(&mut state), 2);
    assert_eq!(state, [42, 1, 42, 3]);
}
