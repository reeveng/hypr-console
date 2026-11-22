// run-rustfix
// edition:2024

fn read_prefix(tokens: &[u8], bound: usize) -> (usize, usize) {
    let mut remaining = tokens.iter().copied();
    let mut reads = 0;
    let mut count = 0usize;

    while count < bound && {
        reads += 1;
        remaining.next().unwrap_or_default()
    } == 1 {
        count = count.wrapping_add(1);
    }
    (count, reads)
}

fn marker(limit: u8, next: impl Fn() -> char) -> u8 {
    let mut n = 0u8;
    while n < limit && '!' != next() {
        n += 1;
    }
    n
}

fn main() {
    assert_eq!(read_prefix(&[1, 1, 0, 1], 8), (2, 3));
    assert_eq!(read_prefix(&[1, 1, 1, 1], 2), (2, 2));
    assert_eq!(read_prefix(&[1], 0), (0, 0));
    assert_eq!(marker(10, || '!'), 0);
    assert_eq!(marker(8, || '.'), 8);
}
