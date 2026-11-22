// run-rustfix
// edition:2024
// rustc-env:CARGO_PKG_RUST_VERSION=1.88

fn zipped(source: &[u8; 4], out: &mut [u8; 6]) {
    for row in 0..4 {
        out[row] = source[row];
        out[row] = out[row].saturating_add(u8::try_from(row).unwrap());
    }
}

fn flattened(source: &[u8; 6], out: &mut [[u8; 2]; 3]) {
    for row in 0..3 {
        out[row][0] = source[usize::wrapping_mul(row, 2).wrapping_add(1)];
        out[row][1] = source[row * 2];
    }
}

fn main() {
    let mut out = [0; 6];
    zipped(&[10, 20, 30, 40], &mut out);
    assert_eq!(out, [10, 21, 32, 43, 0, 0]);
    let mut grid = [[0; 2]; 3];
    flattened(&[1, 2, 3, 4, 5, 6], &mut grid);
    assert_eq!(grid, [[2, 1], [4, 3], [6, 5]]);
}
