// run-rustfix
// edition:2024
#[allow(explicit051_no_machine_width)]
type Position = usize;

fn pack(first: u8, second: u8, third: u8) -> [u8; 3] {
    [first, second, third]
}

fn main() {
    let data = [0_u8, 1, 2, 3, 4, 5, 6, 7];
    for y in 0..2 {
        let lanes = pack(
            data[Position::wrapping_mul(y, 3).wrapping_add(2)],
            data[usize::wrapping_mul(y, 3).wrapping_add(1)],
            data[usize::wrapping_mul(y, 3)],
        );
        assert_eq!(lanes[0], lanes[2] + 2);
    }
}
