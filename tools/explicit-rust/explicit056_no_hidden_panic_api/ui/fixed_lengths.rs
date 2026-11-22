// run-rustfix
// edition:2024

fn copy<const N: usize>(into: &mut [u8; N], from: &[u8; N]) {
    into.copy_from_slice(from);
}

fn nonzero(slice: &[u8], count: usize) -> (usize, usize) {
    (
        slice.chunks(count.max(1)).count(),
        slice.iter().step_by(core::cmp::max(1, count)).count(),
    )
}

fn copy_local(from: [u8; 2]) -> [u8; 2] {
    let mut into = [0; 2];
    assert_eq!(into, [0; 2]);
    into.copy_from_slice(&from);
    into
}

fn main() {
    let mut into = [0, 0];
    copy(&mut into, &[1, 2]);
    assert_eq!(into, [1, 2]);
    assert_eq!(copy_local([1, 2]), [1, 2]);
    assert_eq!(nonzero(&into, 0), (2, 2));
}
