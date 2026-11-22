// run-rustfix
// edition:2024
// rustc-env:CARGO_PKG_RUST_VERSION=1.88

use core::num::Wrapping;

fn generic<const N: usize>(source: &[u8; N], output: &mut [u8; N]) {
    for i in 0..N {
        output[i] = source[i].wrapping_add(1);
    }
}

fn entire_slice(source: &mut [u8]) {
    for i in 0..source.len() {
        source[i] = source[i].wrapping_add(1);
    }
}

fn wrapped_flat(source: [u8; 6], target: &mut [u8; 3]) {
    for i in 0..3 {
        target[i] = source[(Wrapping((Wrapping(i) * Wrapping(2)).0) + Wrapping(1)).0];
        assert!(Wrapping(i).0 < 3);
    }
}

fn shared_uses(source: &[u8; 3], target: &mut [u8; 3]) {
    for i in 0..3 {
        target[i] = source[i].wrapping_add(source.first().copied().unwrap_or_default());
    }
}

fn shifted(source: &[u8; 4], target: &mut [u8; 4]) {
    for i in 1..=2 {
        assert!(i == 1 || i == 2);
        target[i] = source[i];
    }
}

fn window(source: [u8; 8], target: &mut [u8; 8]) {
    fn copy(from: &[u8], to: &mut [u8]) {
        to.copy_from_slice(from);
    }

    for i in 0..2 {
        copy(&source[i * 4 + 1..i * 4 + 3], &mut target[i * 4..i * 4 + 2]);
        target[i * 4 + 3] = source[i * 4 + 3];
    }
}

const SIZE: usize = 3;

fn same_length(source: &[u8; SIZE], target: &mut [u8; SIZE]) {
    for i in 0..SIZE {
        target[i] = source[i];
    }
}

fn parallel(source: [u8; 3], scale: [u8; 3], target: &mut [u8; 3]) {
    for (i, dst) in <[_]>::iter_mut(target).enumerate() {
        *dst = source[i].wrapping_mul(scale[i]);
    }
}

fn main() {
    let mut out = [0; 3];
    generic(&[1, 2, 3], &mut out);
    assert_eq!(out, [2, 3, 4]);
    entire_slice(&mut out);
    assert_eq!(out, [3, 4, 5]);
    wrapped_flat([1, 2, 3, 4, 5, 6], &mut out);
    assert_eq!(out, [2, 4, 6]);
    shared_uses(&[2, 4, 6], &mut out);
    assert_eq!(out, [4, 6, 8]);
    let mut shifted_out = [0; 4];
    shifted(&[1, 2, 3, 4], &mut shifted_out);
    assert_eq!(shifted_out, [0, 2, 3, 0]);
    let mut windows = [0; 8];
    window([1, 2, 3, 4, 5, 6, 7, 8], &mut windows);
    assert_eq!(windows, [2, 3, 0, 4, 6, 7, 0, 8]);
    same_length(&[9, 8, 7], &mut out);
    assert_eq!(out, [9, 8, 7]);
    parallel([1, 2, 3], [2, 3, 4], &mut out);
    assert_eq!(out, [2, 6, 12]);
}
